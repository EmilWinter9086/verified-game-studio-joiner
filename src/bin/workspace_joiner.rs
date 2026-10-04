use verified_studio_joiner::studio_directory::{JoinRequest, StudioDirectory, StudioWorkspace};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let request = JoinRequest {
        company_domain: required(&mut args, "company-domain")?,
        proof: required(&mut args, "txt-proof")?,
        email: required(&mut args, "employee-email")?,
        name: required(&mut args, "employee-name")?,
        workspace: StudioWorkspace {
            workspace_id: required(&mut args, "workspace-id")?,
            asset_namespace: required(&mut args, "asset-namespace")?,
            live_event_channel: required(&mut args, "live-event-channel")?,
            moderation_queue: required(&mut args, "moderation-queue")?,
        },
    };

    let receipt = StudioDirectory::from_env()?
        .verify_and_join(request)
        .await?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

fn required(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing argument: {name}"))
}
