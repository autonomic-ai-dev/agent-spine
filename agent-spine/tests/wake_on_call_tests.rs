use agent_spine::wake_on_call::wait_for_brain;

#[tokio::test]
async fn wait_for_brain_gives_up_immediately_without_attempts() {
    assert!(!wait_for_brain(0).await);
}
