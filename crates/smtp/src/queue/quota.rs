/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::future::Future;

use common::{Server, config::smtp::queue::QueueQuota, expr::functions::ResolveVariable};
use store::{
    ValueKey,
    write::{BatchBuilder, QueueClass, ValueClass},
};
use trc::QueueEvent;

use crate::core::throttle::NewKey;

use super::{Message, QueueEnvelope, QuotaKey, Status};

pub trait HasQueueQuota: Sync + Send {
    fn has_quota(&self, message: &mut Message) -> impl Future<Output = bool> + Send;
    fn check_quota<'x>(
        &'x self,
        quota: &'x QueueQuota,
        envelope: &impl ResolveVariable,
        size: u64,
        id: u64,
        refs: &mut Vec<QuotaKey>,
        session_id: u64,
    ) -> impl Future<Output = bool> + Send;
}

impl HasQueueQuota for Server {
    async fn has_quota(&self, message: &mut Message) -> bool {
        let mut quota_keys = Vec::new();

        if !self.core.smtp.queue.quota.sender.is_empty() {
            for quota in &self.core.smtp.queue.quota.sender {
                if !self
                    .check_quota(
                        quota,
                        message,
                        message.size,
                        0,
                        &mut quota_keys,
                        message.span_id,
                    )
                    .await
                {
                    trc::event!(
                        Queue(QueueEvent::QuotaExceeded),
                        SpanId = message.span_id,
                        Id = quota.id.clone(),
                        Type = "Sender"
                    );

                    return false;
                }
            }
        }

        for quota in &self.core.smtp.queue.quota.rcpt_domain {
            for domain_idx in 0..message.domains.len() {
                if !self
                    .check_quota(
                        quota,
                        &QueueEnvelope::new(message, domain_idx),
                        message.size,
                        ((domain_idx + 1) << 32) as u64,
                        &mut quota_keys,
                        message.span_id,
                    )
                    .await
                {
                    trc::event!(
                        Queue(QueueEvent::QuotaExceeded),
                        SpanId = message.span_id,
                        Id = quota.id.clone(),
                        Type = "Domain"
                    );

                    return false;
                }
            }
        }

        for quota in &self.core.smtp.queue.quota.rcpt {
            for (rcpt_idx, rcpt) in message.recipients.iter().enumerate() {
                if !self
                    .check_quota(
                        quota,
                        &QueueEnvelope::new_rcpt(message, rcpt.domain_idx as usize, rcpt_idx),
                        message.size,
                        (rcpt_idx + 1) as u64,
                        &mut quota_keys,
                        message.span_id,
                    )
                    .await
                {
                    trc::event!(
                        Queue(QueueEvent::QuotaExceeded),
                        SpanId = message.span_id,
                        Id = quota.id.clone(),
                        Type = "Recipient"
                    );

                    return false;
                }
            }
        }

        message.quota_keys = quota_keys;

        true
    }

    async fn check_quota<'x>(
        &'x self,
        quota: &'x QueueQuota,
        envelope: &impl ResolveVariable,
        size: u64,
        id: u64,
        refs: &mut Vec<QuotaKey>,
        session_id: u64,
    ) -> bool {
        if !quota.expr.is_empty()
            && self
                .eval_expr(&quota.expr, envelope, "check_quota", session_id)
                .await
                .unwrap_or(false)
        {
            let key = quota.new_key(envelope, "");
            if let Some(max_size) = quota.size {
                let used_size = self
                    .core
                    .storage
                    .data
                    .get_counter(ValueKey::from(ValueClass::Queue(QueueClass::QuotaSize(
                        key.as_ref().to_vec(),
                    ))))
                    .await
                    .unwrap_or(0) as u64;
                if used_size + size > max_size {
                    return false;
                } else {
                    refs.push(QuotaKey::Size {
                        key: key.as_ref().to_vec(),
                        id,
                    });
                }
            }

            if let Some(max_messages) = quota.messages {
                let total_messages = self
                    .core
                    .storage
                    .data
                    .get_counter(ValueKey::from(ValueClass::Queue(QueueClass::QuotaCount(
                        key.as_ref().to_vec(),
                    ))))
                    .await
                    .unwrap_or(0) as u64;
                if total_messages + 1 > max_messages {
                    return false;
                } else {
                    refs.push(QuotaKey::Count {
                        key: key.as_ref().to_vec(),
                        id,
                    });
                }
            }
        }
        true
    }
}

impl Message {
    pub fn release_quota(&mut self, batch: &mut BatchBuilder) {
        if self.quota_keys.is_empty() {
            return;
        }
        let mut quota_ids = Vec::with_capacity(self.domains.len() + self.recipients.len());
        for (pos, domain) in self.domains.iter().enumerate() {
            if matches!(
                &domain.status,
                Status::Completed(_) | Status::PermanentFailure(_)
            ) {
                quota_ids.push(((pos + 1) as u64) << 32);
            }
        }
        for (pos, rcpt) in self.recipients.iter().enumerate() {
            if matches!(
                &rcpt.status,
                Status::Completed(_) | Status::PermanentFailure(_)
            ) {
                quota_ids.push((pos + 1) as u64);
            }
        }

        if !quota_ids.is_empty() {
            let mut quota_keys = Vec::new();
            for quota_key in std::mem::take(&mut self.quota_keys) {
                match quota_key {
                    QuotaKey::Count { id, key } if quota_ids.contains(&id) => {
                        batch.add(ValueClass::Queue(QueueClass::QuotaCount(key)), -1);
                    }
                    QuotaKey::Size { id, key } if quota_ids.contains(&id) => {
                        batch.add(
                            ValueClass::Queue(QueueClass::QuotaSize(key)),
                            -(self.size as i64),
                        );
                    }
                    _ => {
                        quota_keys.push(quota_key);
                    }
                }
            }
            self.quota_keys = quota_keys;
        }
    }
}
