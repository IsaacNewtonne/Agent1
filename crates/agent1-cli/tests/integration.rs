#[cfg(test)]
mod integration {
    use agent1_core::{new_id, Agent, MemoryItem, ModelConfig, PermissionPolicy};
    use agent1_db::SqliteStore;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    async fn test_db() -> (SqliteStore, PathBuf) {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();
        drop(temp);
        let store = SqliteStore::connect(&path).await.unwrap();
        (store, path)
    }

    #[tokio::test]
    async fn db_agent_crud() {
        let (store, _path) = test_db().await;

        let agent = Agent {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            description: Some("A test agent".to_string()),
            role: Some("assistant".to_string()),
            system_prompt: "You are helpful.".to_string(),
            model: ModelConfig {
                provider: "mock".to_string(),
                model: "final".to_string(),
                base_url: None,
                context_window: 8192,
                temperature: 0.2,
                top_p: None,
                max_tokens: None,
            },
            tools: vec!["file_read".to_string()],
            memory: agent1_core::MemoryConfig::default(),
            permissions: PermissionPolicy::default(),
            max_iterations: 5,
        };

        store.save_agent(&agent).await.unwrap();

        let loaded = store.get_agent("test_agent").await.unwrap();
        assert_eq!(loaded.id, "test_agent");

        let agents = store.list_agents().await.unwrap();
        assert!(!agents.is_empty());
        assert!(agents.iter().any(|a| a.id == "test_agent"));
    }

    #[tokio::test]
    async fn db_session_crud() {
        let (store, _path) = test_db().await;

        let agent = Agent {
            id: "session_test_agent".to_string(),
            name: "Session Test".to_string(),
            description: None,
            role: None,
            system_prompt: "Test".to_string(),
            model: ModelConfig {
                provider: "mock".to_string(),
                model: "final".to_string(),
                base_url: None,
                context_window: 8192,
                temperature: 0.2,
                top_p: None,
                max_tokens: None,
            },
            tools: vec![],
            memory: agent1_core::MemoryConfig::default(),
            permissions: PermissionPolicy::default(),
            max_iterations: 1,
        };
        store.save_agent(&agent).await.unwrap();

        let session = store
            .create_session_shell("session_test_agent", Some("Test Session".to_string()))
            .await
            .unwrap();
        assert_eq!(session.root_agent_id, "session_test_agent");

        let loaded = store.get_session(&session.id).await.unwrap();
        assert_eq!(loaded.id, session.id);

        store
            .update_session_status(&session.id, agent1_core::SessionStatus::Completed)
            .await
            .unwrap();

        let recent = store.recent_sessions(10).await.unwrap();
        assert!(!recent.is_empty());
    }

    #[tokio::test]
    async fn db_memory_crud() {
        let (store, _path) = test_db().await;

        let memory = MemoryItem {
            id: new_id("mem"),
            scope: "agent".to_string(),
            agent_id: Some("test_agent".to_string()),
            content: "Test memory content".to_string(),
            tags: vec!["test".to_string()],
            embedding: None,
            importance: 3,
            created_at: agent1_core::now(),
            updated_at: agent1_core::now(),
        };

        store.write_memory(&memory).await.unwrap();

        let memories = store.search_memories(None, "Test", 10).await.unwrap();
        assert!(!memories.is_empty());
        assert!(memories.iter().any(|m| m.content.contains("Test")));

        store.delete_memory(&memory.id).await.unwrap();

        let after_delete = store.search_memories(None, "Test", 10).await.unwrap();
        assert!(after_delete.is_empty());
    }

    #[tokio::test]
    async fn db_events_and_messages() {
        let (store, _path) = test_db().await;

        let session = store
            .create_session_shell("test_agent", None)
            .await
            .unwrap();

        store
            .save_message(&agent1_core::Message {
                id: new_id("msg"),
                session_id: session.id.clone(),
                from_agent_id: Some("user".to_string()),
                to_agent_id: Some("test_agent".to_string()),
                role: agent1_core::MessageRole::User,
                content: "Hello".to_string(),
                metadata: Default::default(),
                created_at: agent1_core::now(),
            })
            .await
            .unwrap();

        let messages = store.session_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");

        store
            .save_event(&agent1_core::RuntimeEvent {
                id: new_id("evt"),
                session_id: Some(session.id.clone()),
                agent_id: Some("test_agent".to_string()),
                event_type: agent1_core::EventType::SessionStarted,
                payload: serde_json::json!({}),
                created_at: agent1_core::now(),
            })
            .await
            .unwrap();

        let events = store.session_events(&session.id).await.unwrap();
        assert!(!events.is_empty());
    }

    #[tokio::test]
    async fn db_tool_calls() {
        let (store, _path) = test_db().await;

        let session = store
            .create_session_shell("test_agent", None)
            .await
            .unwrap();

        let tool_call = agent1_core::ToolCallRecord {
            id: new_id("tool"),
            session_id: session.id.clone(),
            agent_id: "test_agent".to_string(),
            tool_name: "file_read".to_string(),
            input: serde_json::json!({"path": "test.txt"}),
            output: Some(serde_json::json!("file content")),
            status: agent1_core::ToolCallStatus::Completed,
            error: None,
            started_at: agent1_core::now(),
            finished_at: Some(agent1_core::now()),
        };

        store.save_tool_call(&tool_call).await.unwrap();

        let calls = store.session_tool_calls(&session.id).await.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "file_read");
    }

    #[tokio::test]
    async fn db_mcp_server_crud() {
        let (store, _path) = test_db().await;

        let mcp = agent1_core::McpServerConfig {
            id: new_id("mcp"),
            name: "test_mcp".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: vec!["test".to_string()],
            env: Default::default(),
            enabled: true,
            created_at: agent1_core::now(),
            updated_at: agent1_core::now(),
        };

        store.save_mcp_server(&mcp).await.unwrap();

        let servers = store.list_mcp_servers().await.unwrap();
        assert!(!servers.is_empty());

        store
            .update_mcp_server_enabled("test_mcp", false)
            .await
            .unwrap();

        let loaded = store.get_mcp_server("test_mcp").await.unwrap();
        assert!(!loaded.enabled);

        store.delete_mcp_server("test_mcp").await.unwrap();

        let after_delete = store.list_mcp_servers().await.unwrap();
        assert!(after_delete.is_empty());
    }

    #[tokio::test]
    async fn db_approval_records() {
        let (store, _path) = test_db().await;

        let session = store
            .create_session_shell("test_agent", None)
            .await
            .unwrap();

        let approval = agent1_core::ApprovalRecord {
            id: new_id("approval"),
            session_id: session.id.clone(),
            agent_id: "test_agent".to_string(),
            request: serde_json::json!({"tool": "file_read", "input": {"path": "test.txt"}}),
            decision: Some("approved".to_string()),
            decided_at: Some(agent1_core::now()),
            created_at: agent1_core::now(),
        };

        store.save_approval_request(&approval).await.unwrap();

        let loaded = store.get_approval(&approval.id).await.unwrap();
        assert_eq!(loaded.decision, Some("approved".to_string()));

        let recent = store.recent_approvals(10).await.unwrap();
        assert!(!recent.is_empty());
    }

    #[tokio::test]
    async fn db_agent_cards() {
        let (store, _path) = test_db().await;

        let card = agent1_core::AgentCard {
            id: "test_card".to_string(),
            name: "Test Card".to_string(),
            description: Some("A test card".to_string()),
            skills: vec![agent1_core::AgentSkill {
                name: "test_skill".to_string(),
                description: "Test skill".to_string(),
            }],
            input_modes: vec!["text".to_string()],
            output_modes: vec!["markdown".to_string()],
            endpoint: "http://localhost/agents/test/tasks".to_string(),
        };

        store.save_agent_card(&card).await.unwrap();

        let cards = store.list_agent_cards().await.unwrap();
        assert!(!cards.is_empty());

        let by_skill = store.find_agent_cards_by_skill("test_skill").await.unwrap();
        assert!(!by_skill.is_empty());
    }
}
