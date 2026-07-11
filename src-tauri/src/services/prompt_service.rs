use crate::{
    domain::{DeleteResult, PromptRecord, ReorderPromptsInput, UpsertPromptInput},
    error::AppResult,
    persistence::{
        delete_prompt, get_prompt_by_id, list_prompts, reorder_prompts, upsert_prompt, Database,
    },
};

#[derive(Debug, Clone)]
pub struct PromptService {
    database: Database,
}

impl PromptService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list_prompts(&self) -> AppResult<Vec<PromptRecord>> {
        self.database.read("list_prompts", list_prompts).await
    }

    pub async fn get_prompt(&self, id: String) -> AppResult<PromptRecord> {
        self.database
            .read("get_prompt", move |connection| {
                get_prompt_by_id(connection, id.as_str())
            })
            .await
    }

    pub async fn upsert_prompt(&self, input: UpsertPromptInput) -> AppResult<PromptRecord> {
        self.database
            .write("upsert_prompt", move |connection| {
                upsert_prompt(connection, input)
            })
            .await
    }

    pub async fn delete_prompt(&self, id: String) -> AppResult<DeleteResult> {
        self.database
            .write("delete_prompt", move |connection| {
                delete_prompt(connection, id)
            })
            .await
    }

    pub async fn reorder_prompts(
        &self,
        input: ReorderPromptsInput,
    ) -> AppResult<Vec<PromptRecord>> {
        self.database
            .write("reorder_prompts", move |connection| {
                reorder_prompts(connection, input)
            })
            .await
    }
}
