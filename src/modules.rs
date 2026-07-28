use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const BANK_PREFIX: &str = "bank/";
pub const STAKING_PREFIX: &str = "staking/";
pub const GOVERNANCE_PREFIX: &str = "governance/";
pub const IDENTITY_PREFIX: &str = "identity/";
pub const REWARD_PREFIX: &str = "reward/";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleContext {
    pub chain_id: u64,
    pub height: u64,
    pub state: BTreeMap<String, Vec<u8>>,
    pub events: Vec<String>,
}

impl ModuleContext {
    pub fn put(&mut self, prefix: &str, key: &str, value: Vec<u8>) -> Result<(), String> {
        if !key
            .bytes()
            .all(|item| item.is_ascii_alphanumeric() || b"-_:.".contains(&item))
        {
            return Err("모듈 상태 key에 허용되지 않은 문자가 있습니다.".into());
        }
        self.state.insert(format!("{prefix}{key}"), value);
        Ok(())
    }

    pub fn state_root(&self) -> String {
        let mut hasher = Sha256::new();
        for (key, value) in &self.state {
            hasher.update((key.len() as u64).to_be_bytes());
            hasher.update(key.as_bytes());
            hasher.update((value.len() as u64).to_be_bytes());
            hasher.update(value);
        }
        hex::encode(hasher.finalize())
    }
}

pub trait AppModule {
    fn name(&self) -> &'static str;
    fn prefix(&self) -> &'static str;
    fn validate(&self, payload: &[u8], context: &ModuleContext) -> Result<(), String>;
    fn execute(&self, payload: &[u8], context: &mut ModuleContext) -> Result<(), String>;
}

#[derive(Default)]
pub struct ModuleRouter {
    modules: BTreeMap<String, Box<dyn AppModule + Send + Sync>>,
}

impl ModuleRouter {
    pub fn register<M>(&mut self, module: M) -> Result<(), String>
    where
        M: AppModule + Send + Sync + 'static,
    {
        let name = module.name().to_owned();
        if !module.prefix().ends_with('/') || self.modules.contains_key(&name) {
            return Err("모듈 이름은 고유하고 prefix는 /로 끝나야 합니다.".into());
        }
        self.modules.insert(name, Box::new(module));
        Ok(())
    }

    pub fn dispatch(
        &self,
        module: &str,
        payload: &[u8],
        context: &mut ModuleContext,
    ) -> Result<(), String> {
        let handler = self
            .modules
            .get(module)
            .ok_or("등록되지 않은 모듈입니다.")?;
        handler.validate(payload, context)?;
        handler.execute(payload, context)
    }
}

pub trait StateMigration {
    fn module(&self) -> &'static str;
    fn source_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn migrate(&self, context: &mut ModuleContext) -> Result<(), String>;
}

pub fn apply_migrations(
    module: &str,
    current: u32,
    target: u32,
    migrations: &[Box<dyn StateMigration>],
    context: &mut ModuleContext,
) -> Result<u32, String> {
    let mut version = current;
    while version < target {
        let migration = migrations
            .iter()
            .find(|item| item.module() == module && item.source_version() == version)
            .ok_or("연속된 상태 migration이 없습니다.")?;
        if migration.to_version() <= version {
            return Err("상태 migration 버전은 반드시 증가해야 합니다.".into());
        }
        migration.migrate(context)?;
        version = migration.to_version();
    }
    if version != target {
        return Err("상태 migration이 목표 버전을 지나쳤습니다.".into());
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bank;

    impl AppModule for Bank {
        fn name(&self) -> &'static str {
            "bank"
        }
        fn prefix(&self) -> &'static str {
            BANK_PREFIX
        }
        fn validate(&self, payload: &[u8], _: &ModuleContext) -> Result<(), String> {
            (!payload.is_empty())
                .then_some(())
                .ok_or("빈 payload입니다.".into())
        }
        fn execute(&self, payload: &[u8], context: &mut ModuleContext) -> Result<(), String> {
            context.put(self.prefix(), "last", payload.to_vec())
        }
    }

    #[test]
    fn module_transition_is_deterministic() {
        let mut router = ModuleRouter::default();
        router.register(Bank).unwrap();
        let mut left = ModuleContext::default();
        let mut right = ModuleContext::default();
        router.dispatch("bank", b"transfer", &mut left).unwrap();
        router.dispatch("bank", b"transfer", &mut right).unwrap();
        assert_eq!(left.state_root(), right.state_root());
    }
}
