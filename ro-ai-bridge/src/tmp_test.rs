use rig::tool::{Tool};

pub struct MyTool;

impl Tool for MyTool {
    const NAME: &'static str = "my_tool";
    type Error = String;
    type Args = ();
    type Output = String;
    async fn definition(&self, prompt: String) -> rig::tool::ToolDefinition {
        rig::tool::ToolDefinition {}
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok("".into())
    }
}
