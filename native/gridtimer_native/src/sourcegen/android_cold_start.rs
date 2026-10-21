// v2.23.2.46 - Start restore before composition and move verifier setup off Main.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const ACTIVITY: &str = "com/ofairyo/gridtimer/MainActivity.kt";

pub fn render(path: &str, source: &str) -> Result<String, String> {
    let (old, new) = match path {
        REPOSITORY => (
            "    private val stateDatabase = AppStateDatabase(appContext)",
            r#"    // Helper construction loads the native verifier and reads its key.
    // The IO initialization job must do this, not the Activity/ViewModel constructor.
    private val stateDatabase by lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        AppStateDatabase(appContext)
    }"#,
        ),
        ACTIVITY => (
            "        super.onCreate(savedInstanceState)\n        publishOpenSlotRequest(intent)",
            r#"        super.onCreate(savedInstanceState)
        // Overlap process-scoped IO restore with first composition. Recreated
        // Activities reuse the existing repository and initialization job.
        GridTimerApplication.timerRepository(this)
        publishOpenSlotRequest(intent)"#,
        ),
        _ => return Ok(source.to_owned()),
    };
    if source.matches(old).count() != 1 {
        return Err(format!("cold startup anchor is not unique: {path}"));
    }
    Ok(source.replacen(old, new, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(path: &str) -> String {
        let base = super::super::kotlin_sources::SOURCES
            .iter()
            .find(|source| source.path == path)
            .unwrap();
        render(path, base.contents).unwrap()
    }

    #[test]
    fn restore_starts_before_composition_and_reuses_the_application_repository() {
        let activity = rendered(ACTIVITY);
        let on_create = activity
            .split("override fun onCreate(savedInstanceState: Bundle?)")
            .nth(1)
            .unwrap();
        assert!(
            on_create
                .find("GridTimerApplication.timerRepository(this)")
                .unwrap()
                < on_create.find("setContent {").unwrap()
        );
        assert!(activity.contains(
            "val timerRepository: TimerRepository by lazy(LazyThreadSafetyMode.SYNCHRONIZED)"
        ));
        let repository = rendered(REPOSITORY);
        assert!(repository
            .contains("private val stateDatabase by lazy(LazyThreadSafetyMode.SYNCHRONIZED)"));
        assert!(repository.contains("CoroutineScope(SupervisorJob() + Dispatchers.IO)"));
        assert!(repository.contains("initializationJob.join()"));
    }

    #[test]
    fn changed_anchors_fail_instead_of_silently_shipping_the_slow_path() {
        assert!(render(REPOSITORY, "changed repository").is_err());
        assert!(render(ACTIVITY, "changed activity").is_err());
        assert_eq!(render("unrelated.kt", "untouched").unwrap(), "untouched");
    }
}
