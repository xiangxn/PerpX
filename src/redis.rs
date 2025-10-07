use rustis::client::Client;
use rustis::commands::{ListCommands, StringCommands};
use uuid::Uuid;

pub struct RedisQueue {
    client: Client,
    ttl: usize,
}

impl RedisQueue {
    pub fn new(client: Client, ttl: usize) -> Self {
        Self { client, ttl }
    }

    /// 推送消息到队列
    /// queue_name: 队列名
    /// message: 消息内容
    /// ttl_secs: 消息过期时间（秒）, 默认 60 秒
    pub async fn push(
        &self,
        queue_name: &str,
        message: &str,
        ttl_secs: Option<usize>,
    ) -> anyhow::Result<()> {
        let key = format!("perpx:msg:{}", Uuid::new_v4());

        // 默认 TTL 60 秒
        let ttl = ttl_secs.unwrap_or(self.ttl) as u64;
        // 存消息 + TTL
        self.client.setex(&key, ttl, message).await?;

        // 放入队列
        self.client
            .rpush(format!("perpx:queue:{}", queue_name), &key)
            .await?;

        Ok(())
    }
}
