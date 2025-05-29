use {
    crate::{config::GenesisConfig, framework::CreateMoveVm},
    aptos_native_interface::SafeNativeBuilder,
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_vm_environment::natives::aptos_natives_with_builder,
    move_binary_format::errors::VMError,
    move_vm_runtime::{
        RuntimeEnvironment, WithRuntimeEnvironment, config::VMConfig, move_vm::MoveVM,
    },
};

pub struct UmiVm {
    env: RuntimeEnvironment,
}

impl UmiVm {
    pub fn new(config: &GenesisConfig) -> Self {
        let mut builder = SafeNativeBuilder::new(
            config.gas_costs.version,
            config.gas_costs.natives.clone(),
            config.gas_costs.vm.misc.clone(),
            TimedFeaturesBuilder::enable_all().build(),
            Features::default(),
            None,
        );
        let mut natives = aptos_natives_with_builder(&mut builder, false);
        umi_evm_ext::append_evm_natives(&mut natives, &builder);
        let config = VMConfig {
            paranoid_type_checks: true,
            use_loader_v2: true,
            ..Default::default()
        };
        let env = RuntimeEnvironment::new_with_config(natives, config);
        Self { env }
    }
}

impl WithRuntimeEnvironment for UmiVm {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        &self.env
    }
}

impl WithRuntimeEnvironment for &'_ UmiVm {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        &self.env
    }
}

impl CreateMoveVm for UmiVm {
    fn create_move_vm(&self) -> Result<MoveVM, VMError> {
        let vm = MoveVM::new_with_runtime_environment(&self.env);
        Ok(vm)
    }
}
