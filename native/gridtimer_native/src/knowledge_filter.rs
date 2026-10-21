//! Native, bounded aggregation of knowledge note quick-filter metadata.
//! Text search and workspace scoping happen in Kotlin first; Rust receives
//! only non-sensitive bit masks, never decrypted note text or account IDs.

pub fn quick_filter_counts(flags: &[i32]) -> [i32; 5] {
    let mut counts = [0; 5];
    counts[0] = flags.len() as i32;
    for &flags in flags {
        if flags & 1 != 0 { counts[1] += 1; }
        if flags & 2 != 0 { counts[2] += 1; }
        if flags & 4 != 0 { counts[3] += 1; }
        if flags & 8 != 0 { counts[4] += 1; }
    }
    counts
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_ofairyo_gridtimer_core_NativeOptimizerBridge_nativeKnowledgeQuickFilterCounts(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    flags: jni::objects::JIntArray,
) -> jni::sys::jintArray {
    use std::ptr::null_mut;
    let Ok(length) = env.get_array_length(&flags) else { return null_mut() };
    // UI queries must not allocate unbounded JNI-side arrays.
    if !(0..=250_000).contains(&length) { return null_mut() }
    let mut values = vec![0_i32; length as usize];
    if env.get_int_array_region(&flags, 0, &mut values).is_err() {
        return null_mut();
    }
    let result = quick_filter_counts(&values);
    let Ok(output) = env.new_int_array(result.len() as i32) else {
        return null_mut();
    };
    if env.set_int_array_region(&output, 0, &result).is_err() {
        return null_mut();
    }
    output.into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_and_individual_flags_are_counted_once() {
        assert_eq!(quick_filter_counts(&[]), [0, 0, 0, 0, 0]);
        assert_eq!(quick_filter_counts(&[1, 2, 4, 8, 15, 0]), [6, 2, 2, 2, 2]);
    }

    #[test]
    fn locked_notes_have_no_special_flags_even_when_present() {
        assert_eq!(quick_filter_counts(&[0, 0, 8]), [3, 0, 0, 0, 1]);
    }

    #[test]
    fn repeated_large_collection_has_no_cross_query_state() {
        let flags = vec![15; 20_000];
        assert_eq!(quick_filter_counts(&flags), [20_000; 5]);
        assert_eq!(quick_filter_counts(&[0]), [1, 0, 0, 0, 0]);
    }
}
