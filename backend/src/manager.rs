use std::{collections::HashMap, sync::RwLock};

use async_trait::async_trait;
use eyre::{Context, Result, bail};
use openai_models::{BackendPrompt, Event};
use tokio::sync::mpsc;

use crate::{ArcBackend, Backend};

#[derive(Default)]
pub struct Manager {
    current_model: RwLock<Option<String>>,
    connections: HashMap<String, ArcBackend>, /* Alias - Backend */
    models: HashMap<String, String>,          /* Model - Alias  */
}

impl Manager {
    pub async fn add_connection(&mut self, connection: ArcBackend) -> eyre::Result<()> {
        let alias = connection.name();

        if self.connections.contains_key(&alias) {
            bail!(format!("connection {} already exists", alias))
        }

        connection
            .health_check()
            .await
            .wrap_err(format!("health check connection: {}", alias))?;

        connection
            .list_models(false)
            .await
            .wrap_err("listing model")?
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
    fn name(&self) -> String {
        "Manager".to_string()
    }

    async fn health_check(&self) -> Result<()> {
        for connection in self.connections.values() {
            connection
                .health_check()
                .await
                .wrap_err(format!("health check connection: {}", connection.name()))?;
        }
        Ok(())
    }

    async fn list_models(&self, force: bool) -> Result<Vec<String>> {
        let mut models = Vec::new();
        for connection in self.connections.values() {
            let connection_models = connection.list_models(force).await?;
            models.extend(connection_models);
        }
        Ok(models)
    }

    fn default_model(&self) -> Option<String> {
        let model = self.current_model.read().unwrap();
        model.clone()
    }

    async fn set_default_model(&self, model: &str) -> Result<()> {
        match self.models.keys().filter(|k| k.as_str() == model).next() {
            Some(model) => {
                let mut lock = self.current_model.write().unwrap();
                *lock = Some(model.clone());
                Ok(())
            }
            _ => Err(eyre::eyre!("Model not found")),
        }
    }

    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()> {
        if self.default_model().is_none() {
            return Err(eyre::eyre!("no default model is set"));
        }

        let model = match prompt.model() {
            Some(model) => model.to_string(),
            None => self.default_model().unwrap(),
        };

        let connection = match self.get_connection(&model) {
            Some(connection) => connection,
            None => {
                return Err(eyre::eyre!("Model is not available"));
            }
        };
        connection
            .get_completion(prompt.with_model(&model), event_tx)
            .await
    }
}
