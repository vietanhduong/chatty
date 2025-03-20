#[cfg(test)]
#[path = "manager_test.rs"]
mod tests;

use crate::{ArcBackend, Backend};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use openai_models::{BackendPrompt, Event};
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};

#[derive(Default)]
pub struct Manager {
    current_model: RwLock<Option<String>>,
    connections: HashMap<String, ArcBackend>, /* Alias - Backend */
    models: HashMap<String, String>,          /* Model - Alias  */
}

impl Manager {
    pub async fn add_connection(&mut self, connection: ArcBackend) -> eyre::Result<()> {
        let alias = connection.name().to_string();

        if self.connections.contains_key(&alias) {
            bail!(format!("connection {} already exists", alias))
        }

        connection
            .health_check()
            .await
            .wrap_err(format!("health check backend {}", alias))?;

        connection
            .list_models(false)
            .await
            .wrap_err(format!("listing model backend {}", alias))?
            .into_iter()
            .for_each(|m| {
                self.models.insert(m, alias.clone());
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

    async fn health_check(&self) -> Result<()> {
        for connection in self.connections.values() {
            connection
                .health_check()
                .await
                .wrap_err(format!("health check backend {}", connection.name()))?;
        }
        Ok(())
    }

    async fn list_models(&self, _force: bool) -> Result<Vec<String>> {
        let mut models = self.models.keys().cloned().collect::<Vec<String>>();
        // TODO(vietanhduong): Update the models and connections when force is true
        models.sort();
        Ok(models)
    }

    async fn current_model(&self) -> Option<String> {
        let model = self.current_model.read().await;
        model.clone()
    }

    async fn set_current_model(&self, model: &str) -> Result<()> {
        match self.models.keys().filter(|k| k.as_str() == model).next() {
            Some(model) => {
                let mut lock = self.current_model.write().await;
                *lock = Some(model.clone());
                Ok(())
            }
            _ => Err(eyre::eyre!("model not found")),
        }
    }

    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()> {
        if self.current_model().await.is_none() {
            return Err(eyre::eyre!("no default model is set"));
        }

        let model = match prompt.model() {
            Some(model) => model.to_string(),
            None => self.current_model().await.unwrap(),
        };

        let connection = match self.get_connection(&model) {
            Some(connection) => connection,
            None => {
                return Err(eyre::eyre!("model is not available"));
            }
        };
        connection
            .get_completion(prompt.with_model(&model), event_tx)
            .await
            .wrap_err(format!("get completion from backend {}", connection.name()))?;
        Ok(())
    }
}
