use crate::{
    domain::{CodexProfileRecord, UpsertCodexProfileInput},
    error::AppResult,
    persistence::{list_codex_profiles, upsert_codex_profile, Database},
};

#[derive(Debug, Clone)]
pub struct ProfileService {
    database: Database,
}

impl ProfileService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list_codex_profiles(&self) -> AppResult<Vec<CodexProfileRecord>> {
        self.database
            .read("list_codex_profiles", list_codex_profiles)
            .await
    }

    pub async fn upsert_codex_profile(
        &self,
        input: UpsertCodexProfileInput,
    ) -> AppResult<CodexProfileRecord> {
        self.database
            .write("upsert_codex_profile", move |connection| {
                upsert_codex_profile(connection, input)
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use crate::domain::UpsertCodexProfileInput;

    use super::ProfileService;

    fn unique_db_path(name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "bexo-studio-{name}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn codex_profile_crud_roundtrip() {
        let database = crate::persistence::Database::new(unique_db_path("profile"));
        database.initialize().await.expect("db init");

        let profile_directory =
            env::temp_dir().join(format!("bexo-codex-home-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&profile_directory).expect("create codex directory");

        let service = ProfileService::new(database.clone());
        let profile = service
            .upsert_codex_profile(UpsertCodexProfileInput {
                id: None,
                name: "default-profile".into(),
                description: Some("Primary profile".into()),
                codex_home: profile_directory.display().to_string(),
                startup_mode: "terminal_only".into(),
                resume_strategy: "manual".into(),
                default_args: vec!["--model".into(), "gpt-5".into()],
                is_default: Some(true),
            })
            .await
            .expect("create profile");

        let updated = service
            .upsert_codex_profile(UpsertCodexProfileInput {
                id: Some(profile.id.clone()),
                name: "default-profile".into(),
                description: Some("Updated profile".into()),
                codex_home: profile_directory.display().to_string(),
                startup_mode: "run_codex".into(),
                resume_strategy: "resume_last".into(),
                default_args: vec!["resume".into(), "--last".into()],
                is_default: Some(true),
            })
            .await
            .expect("update profile");

        assert_eq!(updated.startup_mode, "run_codex");

        let profiles = service
            .list_codex_profiles()
            .await
            .expect("list codex profiles");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].default_args, vec!["resume", "--last"]);
    }
}
