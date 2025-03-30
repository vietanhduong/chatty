#[cfg(test)]
#[path = "manager_test.rs"]
mod tests;

use crate::backend::{ArcBackend, Backend};
use crate::models::{ArcEventTx, BackendPrompt, Model};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use std::collections::HashMap;

#[derive(Default)]
pub struct Manager {
    connections: HashMap<String, ArcBackend>, /* Alias - Backend */
    models: HashMap<String, String>,          /* Model ID - Alias  */
}

impl Manager {
    pub async fn add_connection(&mut self, connection: ArcBackend) -> eyre::Result<()> {
        let alias = connection.name().to_string();

        if self.connections.contains_key(&alias) {
            bail!(format!("connection {} already exists", alias))
        }

        connection
            .list_models()
            .await
            .wrap_err(format!("listing models backend {}", alias))?
            .into_iter()
            .for_each(|m| {
                self.models.insert(m.id().to_string(), alias.clone());
            });

        self.connections.insert(alias, connection);
        Ok(())
    }

    pub fn get_connection(&self, model: &str) -> Option<&ArcBackend> {
        let alias = match self.models.get(model) {
            Some(alias) => alias,
            None => return None,
        };
        self.connections.get(alias)
    }
}

#[async_trait]
impl Backend for Manager {
    fn name(&self) -> &str {
        "Manager"
    }

    async fn list_models(&self) -> Result<Vec<Model>> {
        Ok(self
            .models
            .iter()
            .map(|(id, alias)| Model::new(id).with_provider(alias))
            .collect())
    }

    async fn get_completion(&self, prompt: BackendPrompt, event_tx: ArcEventTx) -> Result<()> {
        let connection = match self.get_connection(prompt.model()) {
            Some(connection) => connection,
            None => {
                return Err(eyre::eyre!("model is not available"));
            }
        };
        connection
            .get_completion(prompt, event_tx)
            .await
            .wrap_err(format!("get completion from backend {}", connection.name()))?;
        Ok(())
    }
}
