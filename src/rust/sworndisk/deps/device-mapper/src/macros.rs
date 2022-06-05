//! Macros for block devices & device mapper

/// implement getter and setter of bindgen member for wrapper
///
/// # Examples
///
/// Support we have a Rust wrapper `struct VanillaType` for raw pointer type `*mut vanilla_type`,
/// which contains an member named `attr` of type `T`.
///
/// To safely access the member of `*mut vanilla_type`, we implement getter-setter pattern on
/// the Rust wrapper. The getter and setter return a Result<T>.
///
/// To implement the `pub fn attr(&self) -> Result<T>` and `pub fn set_attr(&mut self) -> Result` on `VanilaType`:
///
/// ```rust
/// impl VanillaType {
///     // generate pub fn attr(&self) -> Result<T>
///     // and pub fn set_sttr(&mut self) -> Result
///     impl_getset!(attr, set_attr, T);
///
///     // generate pub fn get_attr(&self) -> Result<T>
///     // and pub fn set_attr(&mut self) -> Result
///     impl_getset!(attr, get_attr, set_attr, T);
/// }
/// ```
#[macro_export]
macro_rules! impl_getset {
    () => {};

    ($attr_name:ident, $setter_name:ident, $types:ty) => {
        /// Get the value of `$attr_name`
        pub fn $attr_name(&self) -> $types {
            // SAFETY: From the type invariant, we can ensure that `self.inner` is non-null and valid.
            unsafe { (*(self.inner)).$attr_name }
        }

        /// Set the value of `$attr_name`.
        pub fn $setter_name(&mut self, value: $types) {
            // SAFETY: From the type invariant, we can ensure that `self.inner` is non-null and valid.
            unsafe {
                (*(self.inner)).$attr_name = value;
            };
        }
    };

    ($attr_name:ident, $getter_name:ident, $setter_name:ident, $types:ty) => {
        /// Get the value of `$attr_name`.
        pub fn $getter_name(&self) -> $types {
            // SAFETY: From the type invariant, we can ensure that `self.inner` is non-null and valid.
            unsafe { (*(self.inner)).$attr_name }
        }

        /// Set the value of `$attr_name`.
        pub fn $setter_name(&mut self, value: $types) {
            // SAFETY: From the type invariant, we can ensure that `self.inner` is non-null and valid.
            unsafe {
                (*(self.inner)).$attr_name = value;
            };
        }
    };
}

/// Defines the [`Operations::TO_USE`] field based on a list of fields to be populated.
#[macro_export]
macro_rules! declare_device_mapper_callbacks {
    () => {
        const TO_USE: $crate::ToUse = $crate::USE_NONE;
    };
    ($($i:ident),+) => {
        const TO_USE: $crate::ToUse =
            $crate::ToUse {
                $($i: true),+ ,
                ..$crate::USE_NONE
            };
    };
}
