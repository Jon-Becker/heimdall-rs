/// Information about a precompile, such as name, and stack inputs and outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrecompileInfo {
    /// Name
    name: &'static str,
    /// Stack inputs.
    inputs: u8,
    /// Stack outputs.
    outputs: u8,
    /// Minimum gas required to execute the opcode.
    gas: u16,
}

impl PrecompileInfo {
    /// Creates a new precompile with the given name and default values.
    pub const fn new(name: &'static str) -> Self {
        Self { name, inputs: 0, outputs: 0, gas: 0 }
    }

    /// Returns the name of the opcode.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the number of stack inputs.
    #[inline]
    pub const fn inputs(&self) -> u8 {
        self.inputs
    }

    /// Returns the number of stack outputs.
    #[inline]
    pub const fn outputs(&self) -> u8 {
        self.outputs
    }

    /// Returns the minimum gas required to execute the opcode.
    #[inline]
    pub const fn min_gas(&self) -> u16 {
        self.gas
    }
}

impl From<u8> for PrecompileInfo {
    #[inline]
    fn from(precompile: u8) -> Self {
        PRECOMPILE_INFO_JUMPTABLE[precompile as usize].unwrap_or(PrecompileInfo {
            name: "unknown",
            inputs: 0,
            outputs: 0,
            gas: 0,
        })
    }
}

/// Sets the number of stack inputs and outputs.
#[inline]
pub const fn stack_io(mut op: PrecompileInfo, inputs: u8, outputs: u8) -> PrecompileInfo {
    op.inputs = inputs;
    op.outputs = outputs;
    op
}

/// Sets the gas required to execute the opcode.
#[inline]
pub const fn min_gas(mut op: PrecompileInfo, gas: u16) -> PrecompileInfo {
    op.gas = gas;
    op
}

macro_rules! precompiles {
    ($($val:literal => $name:ident => $($modifier:ident $(( $($modifier_arg:expr),* ))?),*);* $(;)?) => {
        // create a constant for each precompile
        $(
            #[doc = concat!("The `", stringify!($val), "` (\"", stringify!($name),"\") precompile.")]
            pub const $name: u8 = $val;
        )*

        /// Maps each opcode to its info.
        pub const PRECOMPILE_INFO_JUMPTABLE: [Option<PrecompileInfo>; 11] = {
            let mut map = [None; 11];
            let mut prev: u8 = 0;
            $(
                let val: u8 = $val;
                assert!(val == 0 || val > prev, "precompiles must be sorted in ascending order");
                prev = val;
                let info = PrecompileInfo::new(
                    stringify!($name)
                );
                $(
                let info = $modifier(info, $($($modifier_arg),*)?);
                )*
                map[$val] = Some(info);
            )*
            let _ = prev;
            map
        };
    }
}

precompiles! {
    0x01 => ECRECOVER => stack_io(4, 1), min_gas(3000);
    0x02 => SHA2_256 => stack_io(1, 1), min_gas(60);
    0x03 => RIPEMD_160 => stack_io(1, 1), min_gas(600);
    0x04 => IDENTITY => stack_io(1, 1), min_gas(15);
    0x05 => MOD_EXP => stack_io(6, 1), min_gas(200);
    0x06 => EC_ADD => stack_io(4, 2), min_gas(150);
    0x07 => EC_MUL => stack_io(3, 2), min_gas(6000);
    0x08 => EC_PAIRING => stack_io(6, 1), min_gas(45000);
    0x09 => BLAKE2_F => stack_io(5, 1), min_gas(0);
    0x0a => POINT_EVAL => stack_io(1, 1), min_gas(0);
}
