use std::sync::Arc;

use anyhow::Result;
use forge_domain::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    AppConfigService, EnvironmentInfra, FileDiscoveryService, ProviderService, TemplateEngine,
    TerminalContextService,
};

/// Response struct for shell command generation using JSON format
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(title = "shell_command")]
pub struct ShellCommandResponse {
    /// The generated shell command
    pub command: String,
}

/// CommandGenerator handles shell command generation from natural language
pub struct CommandGenerator<S> {
    services: Arc<S>,
}

impl<S> CommandGenerator<S>
where
    S: EnvironmentInfra<Config = forge_config::ForgeConfig>
        + FileDiscoveryService
        + ProviderService
        + AppConfigService,
{
    /// Creates a new CommandGenerator instance with the provided services.
    pub fn new(services: Arc<S>) -> Self {
        Self { services }
    }

    /// Generates a shell command from a natural language prompt.
    ///
    /// Terminal context is read automatically from the `_FORGE_TERM_COMMANDS`,
    /// `_FORGE_TERM_EXIT_CODES`, and `_FORGE_TERM_TIMESTAMPS` environment
    /// variables exported by the zsh plugin, and included in the user
    /// prompt so the LLM can reference recent commands, exit codes, and
    /// timestamps.
    pub async fn generate(&self, prompt: UserPrompt) -> Result<String> {
        // Get system information for context
        let env = self.services.get_environment();

        let files = self.services.list_current_directory().await?;

        #[cfg(target_family = "unix")]
        let rendered_system_prompt = TemplateEngine::default().render(
            "sh/forge-command-generator-prompt-sh.md",
            &serde_json::json!({"env": env, "files": files}),
        )?;

        #[cfg(target_os = "windows")]
        let rendered_system_prompt = TemplateEngine::default().render(
            "pwsh/forge-command-generator-prompt-pwsh.md",
            &serde_json::json!({"env": env, "files": files}),
        )?;

        // Get required services and data - use suggest config if available,
        // otherwise fall back to default provider/model
        let (provider, model) = match self.services.get_suggest_config().await? {
            Some(config) => {
                let provider = self.services.get_provider(config.provider).await?;
                (provider, config.model)
            }
            None => {
                let model_config = self
                    .services
                    .get_session_config()
                    .await
                    .ok_or_else(|| Error::NoDefaultSession)?;
                let provider = self.services.get_provider(model_config.provider).await?;
                (provider, model_config.model)
            }
        };

        // Build user prompt with task, optionally including terminal context.
        use forge_template::Element;
        let task_elm = Element::new("task").text(prompt.as_str());
        let terminal_service = TerminalContextService::new(self.services.clone());
        let user_content = match terminal_service.get_terminal_context() {
            Some(ctx) => {
                let terminal_elm =
                    Element::new("command_trace").append(ctx.commands.iter().map(|cmd| {
                        Element::new("command")
                            .attr("exit_code", cmd.exit_code.to_string())
                            .text(&cmd.command)
                    }));
                format!("{}\n\n{}", terminal_elm.render(), task_elm.render())
            }
            None => task_elm.render(),
        };

        // Create context with system and user prompts
        let ctx = self.create_context(rendered_system_prompt, user_content, &model);

        // Send message to LLM
        let stream = self.services.chat(&model, ctx, provider).await?;
        let message = stream.into_full(false).await?;

        // Parse the structured JSON response
        let response: ShellCommandResponse =
            serde_json::from_str(&message.content).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse shell command response: {}. Response: {}",
                    e,
                    message.content
                )
            })?;

        Ok(response.command)
    }

    /// Creates a context with system and user messages for the LLM
    fn create_context(
        &self,
        system_prompt: String,
        user_content: String,
        model: &ModelId,
    ) -> Context {
        // Generate JSON schema from the response struct
        let schema = schemars::schema_for!(ShellCommandResponse);

        Context::default()
            .add_message(ContextMessage::system(system_prompt))
            .add_message(ContextMessage::user(user_content, Some(model.clone())))
            .response_format(ResponseFormat::JsonSchema(Box::new(schema)))
    }
}

#[cfg(test)]
#[path = "../tests/command_generator_test.rs"]
mod test;
