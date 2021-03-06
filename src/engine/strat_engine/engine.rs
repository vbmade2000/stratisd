// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::Path;

use devicemapper::DM;

use super::super::engine::{Engine, HasName, HasUuid, Pool};
use super::super::errors::{EngineError, EngineResult, ErrorEnum};
use super::super::structures::Table;
use super::super::types::{DevUuid, PoolUuid, Redundancy, RenameAction};

use super::cleanup::teardown_pools;
use super::pool::StratPool;
use super::setup::find_all;

#[derive(Debug, PartialEq, Eq)]
pub enum DevOwnership {
    Ours(PoolUuid, DevUuid),
    Unowned,
    Theirs,
}

#[derive(Debug)]
pub struct StratEngine {
    pools: Table<StratPool>,
}

impl StratEngine {
    /// Setup a StratEngine.
    /// 1. Verify the existance of Stratis /dev directory.
    /// 2. Setup all the pools belonging to the engine.
    ///
    /// Returns an error if there was an error reading device nodes.
    /// Returns an error if there was an error setting up any of the pools.
    pub fn initialize() -> EngineResult<StratEngine> {
        let pools = find_all()?;

        let mut table = Table::default();
        for (pool_uuid, devices) in &pools {
            let evicted = table.insert(StratPool::setup(*pool_uuid, devices)?);
            if !evicted.is_empty() {

                // TODO: update state machine on failure.
                let _ = teardown_pools(table.empty());

                let err_msg = "found two pools with the same id or name";
                return Err(EngineError::Engine(ErrorEnum::Invalid, err_msg.into()));
            }
        }

        Ok(StratEngine { pools: table })
    }

    /// Teardown Stratis, preparatory to a shutdown.
    pub fn teardown(self) -> EngineResult<()> {
        teardown_pools(self.pools.empty())
    }
}

impl Engine for StratEngine {
    fn configure_simulator(&mut self, _denominator: u32) -> EngineResult<()> {
        Ok(()) // we're not the simulator and not configurable, so just say ok
    }

    fn create_pool(&mut self,
                   name: &str,
                   blockdev_paths: &[&Path],
                   redundancy: Option<u16>,
                   force: bool)
                   -> EngineResult<PoolUuid> {

        let redundancy = calculate_redundancy!(redundancy);

        if self.pools.contains_name(name) {
            return Err(EngineError::Engine(ErrorEnum::AlreadyExists, name.into()));
        }

        let dm = DM::new()?;
        let pool = StratPool::initialize(name, &dm, blockdev_paths, redundancy, force)?;

        let uuid = pool.uuid();
        self.pools.insert(pool);
        Ok(uuid)
    }

    fn destroy_pool(&mut self, uuid: PoolUuid) -> EngineResult<bool> {
        destroy_pool!{self; uuid}
    }

    fn rename_pool(&mut self, uuid: PoolUuid, new_name: &str) -> EngineResult<RenameAction> {
        let old_name = rename_pool_pre!(self; uuid; new_name);

        let mut pool = self.pools
            .remove_by_uuid(uuid)
            .expect("Must succeed since self.pools.get_by_uuid() returned a value");
        pool.rename(new_name);

        if let Err(err) = pool.write_metadata() {
            pool.rename(&old_name);
            self.pools.insert(pool);
            Err(err)
        } else {
            self.pools.insert(pool);
            Ok(RenameAction::Renamed)
        }
    }

    fn get_pool(&self, uuid: PoolUuid) -> Option<&Pool> {
        get_pool!(self; uuid)
    }

    fn get_mut_pool(&mut self, uuid: PoolUuid) -> Option<&mut Pool> {
        get_mut_pool!(self; uuid)
    }

    fn check(&mut self) -> () {
        check_engine!(self);
    }

    fn pools(&self) -> Vec<&Pool> {
        self.pools.into_iter().map(|x| x as &Pool).collect()
    }
}

#[cfg(test)]
mod test {
    use super::super::tests::{loopbacked, real};

    use super::*;


    /// Verify that a pool rename causes the pool metadata to get the new name.
    fn test_pool_rename(paths: &[&Path]) {
        let mut engine = StratEngine::initialize().unwrap();

        let name1 = "name1";
        let uuid1 = engine.create_pool(&name1, paths, None, false).unwrap();

        let name2 = "name2";
        let action = engine.rename_pool(uuid1, name2).unwrap();

        assert_eq!(action, RenameAction::Renamed);
        engine.teardown().unwrap();

        let engine = StratEngine::initialize().unwrap();
        let pool_name: String = engine.get_pool(uuid1).unwrap().name().into();
        assert_eq!(pool_name, name2);
    }

    #[test]
    pub fn loop_test_pool_rename() {
        loopbacked::test_with_spec(loopbacked::DeviceLimits::Range(1, 3), test_pool_rename);
    }

    #[test]
    pub fn real_test_pool_rename() {
        real::test_with_spec(real::DeviceLimits::AtLeast(1), test_pool_rename);
    }

    /// Test engine setup.
    /// 1. Create two pools.
    /// 2. Verify that both exist.
    /// 3. Teardown the engine.
    /// 4. Verify that pools are gone.
    /// 5. Initialize the engine.
    /// 6. Verify that pools can be found again.
    fn test_setup(paths: &[&Path]) {
        assert!(paths.len() > 1);

        let (paths1, paths2) = paths.split_at(paths.len() / 2);

        let mut engine = StratEngine::initialize().unwrap();

        let name1 = "name1";
        let uuid1 = engine.create_pool(&name1, paths1, None, false).unwrap();

        let name2 = "name2";
        let uuid2 = engine.create_pool(&name2, paths2, None, false).unwrap();

        assert!(engine.get_pool(uuid1).is_some());
        assert!(engine.get_pool(uuid2).is_some());

        engine.teardown().unwrap();

        let engine = StratEngine::initialize().unwrap();

        assert!(engine.get_pool(uuid1).is_some());
        assert!(engine.get_pool(uuid2).is_some());
    }

    #[test]
    pub fn loop_test_setup() {
        loopbacked::test_with_spec(loopbacked::DeviceLimits::Range(2, 3), test_setup);
    }

    #[test]
    pub fn real_test_setup() {
        real::test_with_spec(real::DeviceLimits::AtLeast(2), test_setup);
    }
}
