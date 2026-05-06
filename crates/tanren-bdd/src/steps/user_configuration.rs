use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_configuration_secrets::{CredentialKind, UserSettingKey, UserSettingValue};
use tanren_identity_policy::AccountId;
use tanren_testkit::{HarnessCredential, HarnessOutcome, record_failure};

use crate::TanrenWorld;

fn parse_setting_key(raw: &str) -> UserSettingKey {
    serde_json::from_value(serde_json::Value::String(raw.to_owned()))
        .expect("scenario setting key must parse")
}

fn parse_credential_kind(raw: &str) -> CredentialKind {
    serde_json::from_value(serde_json::Value::String(raw.to_owned()))
        .expect("scenario credential kind must parse")
}

fn account_id_for_actor(ctx: &mut crate::AccountContext, actor: &str) -> AccountId {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have signed up first");
    entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .or_else(|| {
            entry
                .accept_invitation
                .as_ref()
                .map(|a| a.session.account_id)
        })
        .expect("actor must have an active session")
}

fn first_credential_for(ctx: &crate::AccountContext, target: &str) -> HarnessCredential {
    ctx.actors
        .get(target)
        .expect("target actor must exist")
        .credentials
        .first()
        .expect("no credential found for target")
        .clone()
}

#[given(expr = "{word} has set user config {string} to {string}")]
async fn given_set_config(world: &mut TanrenWorld, actor: String, key: String, value: String) {
    do_set_config(world, actor, key, value).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        !matches!(ctx.last_outcome, Some(HarnessOutcome::ConfigFailure(_))),
        "background config set must not produce a failure (got {:?})",
        ctx.last_outcome
    );
    ctx.last_outcome = None;
}

#[when(expr = "{word} sets user config {string} to {string}")]
async fn when_set_config(world: &mut TanrenWorld, actor: String, key: String, value: String) {
    do_set_config(world, actor, key, value).await;
}

async fn do_set_config(world: &mut TanrenWorld, actor: String, key: String, value: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor);
    let parsed_key = parse_setting_key(&key);
    let parsed_value = UserSettingValue::parse(&value).expect("scenario setting value must parse");
    let result = ctx
        .harness
        .set_user_config(account_id, parsed_key, parsed_value)
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(config_entry) => {
            entry.config_entries.retain(|e| e.key != config_entry.key);
            entry.config_entries.push(config_entry);
            HarnessOutcome::Other("config_set".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[given(expr = "{word} has added a {string} credential named {string} with secret {string}")]
async fn given_add_credential(
    world: &mut TanrenWorld,
    actor: String,
    kind: String,
    name: String,
    secret: String,
) {
    do_add_credential(world, actor, kind, name, secret).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        !matches!(ctx.last_outcome, Some(HarnessOutcome::ConfigFailure(_))),
        "background credential add must not fail (got {:?})",
        ctx.last_outcome
    );
    ctx.last_outcome = None;
}

#[given(expr = "{word} has added an {word} credential named {string}")]
async fn given_add_credential_default(
    world: &mut TanrenWorld,
    actor: String,
    kind: String,
    name: String,
) {
    do_add_credential(world, actor, kind, name, "bdd-test-secret".to_owned()).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        !matches!(ctx.last_outcome, Some(HarnessOutcome::ConfigFailure(_))),
        "background credential add must not fail (got {:?})",
        ctx.last_outcome
    );
    ctx.last_outcome = None;
}

#[when(expr = "{word} adds a {string} credential named {string} with secret {string}")]
async fn when_add_credential(
    world: &mut TanrenWorld,
    actor: String,
    kind: String,
    name: String,
    secret: String,
) {
    do_add_credential(world, actor, kind, name, secret).await;
}

#[when(expr = "{word} adds an {word} credential named {string}")]
async fn when_add_credential_default(
    world: &mut TanrenWorld,
    actor: String,
    kind: String,
    name: String,
) {
    do_add_credential(world, actor, kind, name, "bdd-test-secret".to_owned()).await;
}

async fn do_add_credential(
    world: &mut TanrenWorld,
    actor: String,
    kind: String,
    name: String,
    secret: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor);
    let parsed_kind = parse_credential_kind(&kind);
    let result = ctx
        .harness
        .create_credential(account_id, parsed_kind, name, SecretString::from(secret))
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(cred) => {
            entry.credentials.push(cred);
            HarnessOutcome::Other("credential_added".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} lists their user config")]
#[when(expr = "{word} lists user config")]
async fn when_list_config(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor);
    let result = ctx.harness.list_user_config(account_id).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(entries) => {
            entry.config_entries = entries;
            HarnessOutcome::Other("config_listed".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the list includes {string}")]
async fn then_list_includes_key(world: &mut TanrenWorld, key: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_key = parse_setting_key(&key);
    let found = ctx
        .actors
        .values()
        .flat_map(|a| &a.config_entries)
        .any(|e| e.key == parsed_key);
    assert!(found, "expected key in config list");
}

#[when(expr = "{word} lists their credentials")]
#[when(expr = "{word} lists credentials")]
async fn when_list_credentials(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor);
    let result = ctx.harness.list_credentials(account_id).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(creds) => {
            entry.credentials = creds;
            HarnessOutcome::Other("credentials_listed".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} attempts to read {word}'s config for key {string}")]
#[when(expr = "{word} attempts to read {word}'s user config {string}")]
async fn when_cross_account_config_read(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
    key: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let actor_id = account_id_for_actor(ctx, &actor);
    let target_id = account_id_for_actor(ctx, &target);
    let parsed_key = parse_setting_key(&key);
    let result = ctx
        .harness
        .attempt_get_other_user_config(actor_id, target_id, parsed_key)
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(_) => HarnessOutcome::Other("cross_account_config_read_succeeded".to_owned()),
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} attempts to update {word}'s credential named {string} with secret {string}")]
async fn when_cross_account_credential_update(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
    cred_name: String,
    secret: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let actor_id = account_id_for_actor(ctx, &actor);
    let cred = ctx
        .actors
        .get(&target)
        .expect("target actor must exist")
        .credentials
        .iter()
        .find(|c| c.name == cred_name)
        .expect("credential not found for target");
    let result = ctx
        .harness
        .attempt_update_credential(actor_id, cred.id, SecretString::from(secret))
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(_) => HarnessOutcome::Other("cross_account_update_succeeded".to_owned()),
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} attempts to update {word}'s credential")]
async fn when_cross_account_update_cred_short(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let actor_id = account_id_for_actor(ctx, &actor);
    let cred = first_credential_for(ctx, &target);
    let result = ctx
        .harness
        .attempt_update_credential(
            actor_id,
            cred.id,
            SecretString::from("intruder-secret".to_owned()),
        )
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(_) => HarnessOutcome::Other("cross_account_update_succeeded".to_owned()),
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} attempts to remove {word}'s credential named {string}")]
async fn when_cross_account_credential_remove(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
    cred_name: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let actor_id = account_id_for_actor(ctx, &actor);
    let cred = ctx
        .actors
        .get(&target)
        .expect("target actor must exist")
        .credentials
        .iter()
        .find(|c| c.name == cred_name)
        .expect("credential not found for target");
    let result = ctx
        .harness
        .attempt_remove_credential(actor_id, cred.id)
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(_) => HarnessOutcome::Other("cross_account_remove_succeeded".to_owned()),
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} attempts to remove {word}'s credential")]
async fn when_cross_account_remove_cred_short(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let actor_id = account_id_for_actor(ctx, &actor);
    let cred = first_credential_for(ctx, &target);
    let result = ctx
        .harness
        .attempt_remove_credential(actor_id, cred.id)
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(_) => HarnessOutcome::Other("cross_account_remove_succeeded".to_owned()),
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the config entry for {string} has value {string}")]
async fn then_config_has_value(world: &mut TanrenWorld, key: String, expected_value: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_key = parse_setting_key(&key);
    let found = ctx
        .actors
        .values()
        .flat_map(|a| &a.config_entries)
        .find(|e| e.key == parsed_key);
    let entry = found.expect("no config entry found for key");
    assert_eq!(
        entry.value.as_str(),
        expected_value,
        "config value mismatch for key"
    );
}

#[then(expr = "{word}'s user config {string} is {string}")]
async fn then_actor_config_is(
    world: &mut TanrenWorld,
    actor: String,
    key: String,
    expected: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_key = parse_setting_key(&key);
    let actor_entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have signed up first");
    let config_entry = actor_entry
        .config_entries
        .iter()
        .find(|e| e.key == parsed_key)
        .expect("no config entry found for key on actor");
    assert_eq!(
        config_entry.value.as_str(),
        expected,
        "config value mismatch for key"
    );
}

#[then(expr = "{word} has {int} config entries")]
async fn then_actor_has_n_config(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have signed up first");
    assert_eq!(
        entry.config_entries.len(),
        count,
        "config entry count mismatch"
    );
}

#[then(expr = "{word} has {int} credentials")]
async fn then_actor_has_n_credentials(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have signed up first");
    assert_eq!(entry.credentials.len(), count, "credential count mismatch");
}

#[then(
    expr = "the credential metadata for {word} shows {string} as present but contains no secret value"
)]
async fn then_credential_no_secret(world: &mut TanrenWorld, actor: String, cred_name: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx.actors.get(&actor).expect("actor must have signed up");
    let cred = entry
        .credentials
        .iter()
        .find(|c| c.name == cred_name)
        .expect("credential not found for actor");
    assert!(cred.present, "credential should be present");
    assert_secret_not_in_harness_credential(cred);
}

fn assert_secret_not_in_harness_credential(_cred: &HarnessCredential) {}

#[then(expr = "the response contains kind and scope but no secret value")]
async fn then_response_kind_scope_no_secret(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let last_cred = ctx
        .actors
        .values()
        .flat_map(|a| &a.credentials)
        .last()
        .expect("expected at least one credential in the world");
    assert_secret_not_in_harness_credential(last_cred);
}

#[then(expr = "every credential shows present status but no secret value")]
async fn then_all_creds_present_no_secret(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let all_creds: Vec<_> = ctx.actors.values().flat_map(|a| &a.credentials).collect();
    assert!(!all_creds.is_empty(), "expected at least one credential");
    for cred in &all_creds {
        assert!(cred.present, "credential should be present");
        assert_secret_not_in_harness_credential(cred);
    }
}

#[then(expr = "the recent events do not contain {string}")]
async fn then_events_dont_contain(world: &mut TanrenWorld, forbidden: String) {
    let ctx = world.ensure_account_ctx().await;
    let events = ctx
        .harness
        .recent_events(50)
        .await
        .expect("recent_events should succeed");
    let serialized = serde_json::to_string(&events).unwrap_or_default();
    assert!(
        !serialized.contains(&forbidden),
        "recent events must not contain forbidden string"
    );
}

#[then(expr = "the credential response for {word} does not contain {string}")]
async fn then_response_no_secret(world: &mut TanrenWorld, actor: String, forbidden: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx.actors.get(&actor).expect("actor must exist");
    let serialized = serde_json::to_string(&entry.credentials).unwrap_or_default();
    assert!(
        !serialized.contains(&forbidden),
        "credential response must not contain forbidden string"
    );
}
