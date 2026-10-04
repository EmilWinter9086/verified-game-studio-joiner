use verified_studio_joiner::studio_directory::{
    decide_join, JoinDecision, JoinRequest, StudioWorkspace,
};

fn request(email: &str) -> JoinRequest {
    JoinRequest {
        email: email.to_owned(),
        name: "Rin Park".to_owned(),
        company_domain: "emberforge.games".to_owned(),
        proof: "studio-verification=wf_7".to_owned(),
        workspace: StudioWorkspace {
            workspace_id: "ws_liveops".to_owned(),
            asset_namespace: "ugc/emberforge".to_owned(),
            live_event_channel: "season-12".to_owned(),
            moderation_queue: "player-assets".to_owned(),
        },
    }
}

#[test]
fn only_a_matching_company_domain_enters_the_workspace() {
    assert_eq!(
        decide_join(&request("rin@emberforge.games")),
        JoinDecision::Join {
            email: "rin@emberforge.games".to_owned(),
            workspace_id: "ws_liveops".to_owned(),
        }
    );
    assert_eq!(
        decide_join(&request("rin@contractor.example")),
        JoinDecision::RejectDomainMismatch
    );
}
