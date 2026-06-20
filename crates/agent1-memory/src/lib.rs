use agent1_core::{ModelConfig, Suggestion, SuggestionStatus};
use agent1_models::provider_for;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub memory_type: MemoryType,
    pub importance: f32,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u32,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Conversation,
    Fact,
    Preference,
    Pattern,
    Task,
    Artifact,
}

impl Default for MemoryType {
    fn default() -> Self {
        Self::Conversation
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Conversation => write!(f, "conversation"),
            MemoryType::Fact => write!(f, "fact"),
            MemoryType::Preference => write!(f, "preference"),
            MemoryType::Pattern => write!(f, "pattern"),
            MemoryType::Task => write!(f, "task"),
            MemoryType::Artifact => write!(f, "artifact"),
        }
    }
}

impl From<&str> for MemoryType {
    fn from(s: &str) -> Self {
        match s {
            "conversation" => MemoryType::Conversation,
            "fact" => MemoryType::Fact,
            "preference" => MemoryType::Preference,
            "pattern" => MemoryType::Pattern,
            "task" => MemoryType::Task,
            "artifact" => MemoryType::Artifact,
            _ => MemoryType::Conversation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchQuery {
    pub query: String,
    pub memory_type: Option<MemoryType>,
    pub limit: usize,
    pub min_relevance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub entry: MemoryEntry,
    pub similarity: f32,
}

#[async_trait]
pub trait MemoryProvider: Send + Sync {
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    async fn retrieve(&self, id: &str) -> Result<Option<MemoryEntry>>;
    async fn search(&self, query: MemorySearchQuery) -> Result<Vec<MemorySearchResult>>;
    async fn update_access(&self, id: &str) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn list(&self, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryEntry>>;
    async fn consolidate(&self, model_config: &ModelConfig) -> Result<u32>;
}

pub struct SemanticMemoryStore {
    pool: sqlx::SqlitePool,
}

impl SemanticMemoryStore {
    pub async fn connect(db_path: impl AsRef<Path>) -> Result<Self> {
        let connection_string = format!(
            "sqlite:{}?mode=rwc",
            db_path.as_ref().display()
        );
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to memory database: {}", e))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS semantic_memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                embedding BLOB NOT NULL,
                memory_type TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 0.5,
                created_at TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 1,
                metadata TEXT NOT NULL DEFAULT '{}'
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create memories table: {}", e))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_memories_type ON semantic_memories(memory_type)
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create type index: {}", e))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_memories_accessed ON semantic_memories(last_accessed)
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create accessed index: {}", e))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS suggestions (
                id TEXT PRIMARY KEY,
                suggestion_type TEXT NOT NULL,
                content TEXT NOT NULL,
                trigger_context TEXT NOT NULL DEFAULT '',
                related_memory_id TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                accepted_at TEXT,
                dismissed_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create suggestions table: {}", e))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_suggestions_status ON suggestions(status)
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create status index: {}", e))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_suggestions_type ON suggestions(suggestion_type)
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create type index: {}", e))?;

        Ok(Self { pool })
    }

    pub async fn embed_text(&self, text: &str, model_config: &ModelConfig) -> Result<Vec<f32>> {
        let provider = provider_for(model_config)?;
        provider
            .embeddings(text, model_config)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub async fn store_suggestion(&self, suggestion: &Suggestion) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO suggestions
            (id, suggestion_type, content, trigger_context, related_memory_id, status, created_at, updated_at, accepted_at, dismissed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&suggestion.id)
        .bind(suggestion.suggestion_type.to_string())
        .bind(&suggestion.content)
        .bind(&suggestion.trigger_context)
        .bind(&suggestion.related_memory_id)
        .bind(suggestion.status.to_string())
        .bind(suggestion.created_at.to_rfc3339())
        .bind(suggestion.updated_at.to_rfc3339())
        .bind(suggestion.accepted_at.map(|t| t.to_rfc3339()))
        .bind(suggestion.dismissed_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to store suggestion: {}", e))?;

        Ok(())
    }

    pub async fn get_suggestions(&self, status: Option<SuggestionStatus>, limit: usize) -> Result<Vec<Suggestion>> {
        let status_filter = status.map(|s| s.to_string());

        let rows: Vec<(
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = match &status_filter {
            Some(_) => {
                sqlx::query_as(
                    r#"
                    SELECT id, suggestion_type, content, trigger_context, related_memory_id, status, created_at, updated_at, accepted_at, dismissed_at
                    FROM suggestions
                    WHERE status = ?
                    ORDER BY created_at DESC
                    LIMIT ?
                    "#,
                )
                .bind(&status_filter)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get suggestions: {}", e))?
            }
            None => {
                sqlx::query_as(
                    r#"
                    SELECT id, suggestion_type, content, trigger_context, related_memory_id, status, created_at, updated_at, accepted_at, dismissed_at
                    FROM suggestions
                    ORDER BY created_at DESC
                    LIMIT ?
                    "#,
                )
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get suggestions: {}", e))?
            }
        };

        Ok(rows
            .into_iter()
            .map(|(id, suggestion_type, content, trigger_context, related_memory_id, status, created_at, updated_at, accepted_at, dismissed_at)| {
                Suggestion {
                    id,
                    suggestion_type: parse_enum(&suggestion_type),
                    content,
                    trigger_context,
                    related_memory_id,
                    status: parse_enum(&status),
                    created_at: parse_date(&created_at),
                    updated_at: parse_date(&updated_at),
                    accepted_at: accepted_at.and_then(|s| parse_date_opt(&s)),
                    dismissed_at: dismissed_at.and_then(|s| parse_date_opt(&s)),
                }
            })
            .collect())
    }

    pub async fn update_suggestion_status(&self, id: &str, status: SuggestionStatus) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let accepted_at_val: Option<String>;
        let dismissed_at_val: Option<String>;
        match status {
            SuggestionStatus::Accepted => {
                accepted_at_val = Some(now.clone());
                dismissed_at_val = None;
            }
            SuggestionStatus::Dismissed => {
                accepted_at_val = None;
                dismissed_at_val = Some(now.clone());
            }
            _ => {
                accepted_at_val = None;
                dismissed_at_val = None;
            }
        };

        sqlx::query(
            r#"
            UPDATE suggestions
            SET status = ?, updated_at = ?, accepted_at = ?, dismissed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.to_string())
        .bind(&now)
        .bind(accepted_at_val)
        .bind(dismissed_at_val)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update suggestion status: {}", e))?;

        Ok(())
    }

    pub async fn delete_suggestion(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM suggestions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete suggestion: {}", e))?;
        Ok(())
    }
}

fn blob_to_embedding(blob: Vec<u8>) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn row_to_entry(
    id: String,
    content: String,
    embedding: Vec<u8>,
    memory_type: String,
    importance: f32,
    created_at: String,
    last_accessed: String,
    access_count: i64,
    metadata: String,
) -> MemoryEntry {
    MemoryEntry {
        id,
        content,
        embedding: blob_to_embedding(embedding),
        memory_type: MemoryType::from(memory_type.as_str()),
        importance,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_accessed: DateTime::parse_from_rfc3339(&last_accessed)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        access_count: access_count as u32,
        metadata: serde_json::from_str(&metadata).unwrap_or_default(),
    }
}

#[async_trait]
impl MemoryProvider for SemanticMemoryStore {
    async fn store(&self, entry: MemoryEntry) -> Result<()> {
        let embedding_bytes = embedding_to_blob(&entry.embedding);

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO semantic_memories
            (id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&entry.id)
        .bind(&entry.content)
        .bind(&embedding_bytes)
        .bind(entry.memory_type.to_string())
        .bind(entry.importance)
        .bind(entry.created_at.to_rfc3339())
        .bind(entry.last_accessed.to_rfc3339())
        .bind(entry.access_count)
        .bind(&entry.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to store memory: {}", e))?;

        Ok(())
    }

    async fn retrieve(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let row: Option<(String, String, Vec<u8>, String, f32, String, String, i64, String)> =
            sqlx::query_as(
                r#"
                SELECT id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata
                FROM semantic_memories WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to retrieve memory: {}", e))?;

        Ok(row.map(|(id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata)| {
            row_to_entry(id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata)
        }))
    }

    async fn search(&self, query: MemorySearchQuery) -> Result<Vec<MemorySearchResult>> {
        let query_embedding = self
            .embed_text(&query.query, &ModelConfig {
                provider: "ollama".to_string(),
                model: "llama3.1:8b".to_string(),
                base_url: Some("http://localhost:11434".to_string()),
                context_window: 8192,
                temperature: 0.0,
                top_p: None,
                max_tokens: None,
            })
            .await?;

        let type_filter = query
            .memory_type
            .map(|mt| format!("AND memory_type = '{}'", mt.to_string()));

        let sql = format!(
            r#"
            SELECT id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata
            FROM semantic_memories
            WHERE 1=1
            {}
            LIMIT 100
            "#,
            type_filter.as_deref().unwrap_or("")
        );

        let rows: Vec<(String, String, Vec<u8>, String, f32, String, String, i64, String)> =
            sqlx::query_as(&sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to search memories: {}", e))?;

        let mut results: Vec<MemorySearchResult> = rows
            .into_iter()
            .filter_map(|(id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata)| {
                let embedding_vec = blob_to_embedding(embedding.clone());
                let similarity = cosine_similarity(&query_embedding, &embedding_vec);

                if similarity < query.min_relevance {
                    return None;
                }

                Some(MemorySearchResult {
                    entry: row_to_entry(id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata),
                    similarity,
                })
            })
            .collect();

        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(query.limit);
        Ok(results)
    }

    async fn update_access(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE semantic_memories
            SET last_accessed = ?, access_count = access_count + 1
            WHERE id = ?
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update memory access: {}", e))?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM semantic_memories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete memory: {}", e))?;
        Ok(())
    }

    async fn list(&self, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryEntry>> {
        let type_filter = memory_type.map(|mt| format!("WHERE memory_type = '{}'", mt.to_string()));

        let sql = format!(
            r#"
            SELECT id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata
            FROM semantic_memories
            {}
            ORDER BY last_accessed DESC
            LIMIT ?
            "#,
            type_filter.as_deref().unwrap_or("WHERE 1=1")
        );

        let rows: Vec<(String, String, Vec<u8>, String, f32, String, String, i64, String)> =
            sqlx::query_as(&sql)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to list memories: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|(id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata)| {
                row_to_entry(id, content, embedding, memory_type, importance, created_at, last_accessed, access_count, metadata)
            })
            .collect())
    }

    async fn consolidate(&self, model_config: &ModelConfig) -> Result<u32> {
        let old_memories: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT id, content, memory_type FROM semantic_memories
            WHERE access_count < 3 AND created_at < datetime('now', '-7 days')
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to query old memories: {}", e))?;

        if old_memories.is_empty() {
            return Ok(0);
        }

        let type_priority = |mt: &str| match mt {
            "preference" => 0,
            "pattern" => 1,
            "fact" => 2,
            "task" => 3,
            "conversation" => 4,
            "artifact" => 5,
            _ => 6,
        };

        let mut sorted_memories = old_memories;
        sorted_memories.sort_by(|a, b| type_priority(&a.2).cmp(&type_priority(&b.2)));

        let mut consolidated_count = 0u32;

        for chunk in sorted_memories.chunks(10) {
            let combined: String = chunk.iter().map(|(_, c, _)| c as &str).collect::<Vec<_>>().join("\n---\n");
            let prompt = format!(
                "Summarize the following memories into a single concise fact or pattern. Keep only the most important information. Return only the summary, no explanation.\n\n{}",
                combined
            );

            match self.embed_text(&prompt, model_config).await {
                Ok(embedding) => {
                    let id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string();
                    let now = Utc::now();

                    let entry = MemoryEntry {
                        id,
                        content: prompt,
                        embedding,
                        memory_type: MemoryType::Pattern,
                        importance: 0.3,
                        created_at: now,
                        last_accessed: now,
                        access_count: 0,
                        metadata: serde_json::json!({"consolidated": true, "source_count": chunk.len()}),
                    };

                    self.store(entry).await?;
                    consolidated_count += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to embed consolidated memory: {}", e);
                }
            }
        }

        for (id, _, _) in sorted_memories.iter().take(consolidated_count as usize * 10) {
            let _ = sqlx::query("DELETE FROM semantic_memories WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await;
        }

        Ok(consolidated_count)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}

fn parse_enum<T: std::str::FromStr + ToString>(s: &str) -> T
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    s.parse::<T>().unwrap_or_else(|_| {
        s.to_lowercase()
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().to_string() + chars.as_str()
                    }
                }
            })
            .collect::<String>()
            .parse()
            .expect("failed to parse enum")
    })
}

fn parse_date(s: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn parse_date_opt(s: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn create_memory_entry(
    content: String,
    embedding: Vec<f32>,
    memory_type: MemoryType,
    importance: f32,
) -> MemoryEntry {
    let now = Utc::now();
    MemoryEntry {
        id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string(),
        content,
        embedding,
        memory_type,
        importance: importance.clamp(0.0, 1.0),
        created_at: now,
        last_accessed: now,
        access_count: 0,
        metadata: serde_json::json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![0.707, 0.707, 0.0];
        assert!((cosine_similarity(&a, &d) - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_memory_type_display() {
        assert_eq!(MemoryType::Conversation.to_string(), "conversation");
        assert_eq!(MemoryType::Preference.to_string(), "preference");
        assert_eq!(MemoryType::Pattern.to_string(), "pattern");
    }

    #[test]
    fn test_memory_type_from_str() {
        assert_eq!(MemoryType::from("conversation"), MemoryType::Conversation);
        assert_eq!(MemoryType::from("preference"), MemoryType::Preference);
        assert_eq!(MemoryType::from("unknown"), MemoryType::Conversation);
    }

    #[test]
    fn test_create_memory_entry() {
        let entry = create_memory_entry(
            "Test content".to_string(),
            vec![0.1, 0.2, 0.3],
            MemoryType::Fact,
            0.8,
        );
        assert_eq!(entry.content, "Test content");
        assert_eq!(entry.memory_type, MemoryType::Fact);
        assert!((entry.importance - 0.8).abs() < 0.001);
        assert!(!entry.id.is_empty());
    }

    #[test]
    fn test_blob_conversion() {
        let original = vec![0.1f32, 0.2, 0.3, 0.4];
        let blob = embedding_to_blob(&original);
        let recovered = blob_to_embedding(blob);
        assert_eq!(original.len(), recovered.len());
        for (o, r) in original.iter().zip(recovered.iter()) {
            assert!((o - r).abs() < 0.001);
        }
    }
}