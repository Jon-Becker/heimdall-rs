#[cfg(test)]
mod test_logging {
    use std::time::Instant;

    use crate::io::logging::Logger;

    #[test]
    fn test_raw_trace() {
        let start_time = Instant::now();
        let (logger, mut trace) = Logger::new("TRACE");

        let parent = trace.add("call", 0, 123123, vec!["Test::test_trace()".to_string()]);
        trace.add(
            "log",
            parent,
            234234,
            vec!["ContractCreated(contractAddress: 0x0000000000000000000000000000000000000000)"
                .to_string()],
        );
        let inner = trace.add(
            "create",
            parent,
            121234,
            vec![
                "TestContract".to_string(),
                "0x0000000000000000000000000000000000000000".to_string(),
                "917".to_string(),
            ],
        );
        trace.add(
            "log_unknown",
            inner,
            12344,
            vec!["0x0000000000000000000000000000000000000000000000000000000000000000".to_string()],
        );
        let deeper = trace.add(
            "call",
            inner,
            12344,
            vec![
                "Test::transfer(to: 0x0000000000000000000000000000000000000000, amount: 1)"
                    .to_string(),
                "true".to_string(),
            ],
        );
        trace.add("log", deeper, 12344, vec!["Transfer(from: 0x0000000000000000000000000000000000000000, to: 0x0000000000000000000000000000000000000000, amount: 1)".to_string()]);
        trace.add("message", inner, 12344, vec!["warn: Transfer to the zero address!".to_string()]);
        trace.add(
            "message",
            parent,
            12344,
            vec![
                "Execution Reverted: Out of Gas.".to_string(),
                "Execution Reverted: Out of Gas.".to_string(),
            ],
        );

        trace.display();
        logger.info(&format!("Tracing took {}", start_time.elapsed().as_secs_f64()));
    }

    #[test]
    fn test_helper_functions() {
        let start_time = Instant::now();
        let (logger, mut trace) = Logger::new("TRACE");

        let parent = trace.add_call(
            0,
            123,
            "Test".to_string(),
            "test_trace".to_string(),
            vec!["arg1: 0x0".to_string(), "arg2: 0x1".to_string()],
            "()".to_string(),
        );
        trace.add_creation(
            parent,
            124,
            "TestContract".to_string(),
            "0x0000000000000000000000000000000000000000".to_string(),
            1232,
        );
        trace.add_emission(
            parent,
            125,
            "ContractCreated".to_string(),
            vec!["contractAddress: 0x0000000000000000000000000000000000000000".to_string()],
        );
        trace.add_raw_emission(
            parent,
            125,
            vec![
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ],
            "0x".to_string(),
        );
        trace.add_error(parent, 126, "Testing errors");
        trace.add_info(parent, 127, "Testing info");
        trace.add_message(
            parent,
            128,
            vec!["test multiple".to_string(), "lines".to_string(), "to tracing".to_string()],
        );

        trace.display();
        logger.info(&format!("Tracing took {}", start_time.elapsed().as_secs_f64()));
    }

    #[test]
    fn test_option() {
        let (logger, _) = Logger::new("TRACE");

        logger.option(
            "warn",
            "multiple possibilities",
            vec!["option 1".to_string(), "option 2".to_string(), "option 3".to_string()],
            Some(0),
            true,
        );
    }

    #[test]
    fn test_warn() {
        let (logger, _) = Logger::new("SILENT");
        logger.warn("log");

        let (logger, _) = Logger::new("ERROR");
        logger.warn("log");

        let (logger, _) = Logger::new("WARN");
        logger.warn("log");

        let (logger, _) = Logger::new("INFO");
        logger.warn("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.warn("log");

        let (logger, _) = Logger::new("TRACE");
        logger.warn("log");

        let (logger, _) = Logger::new("ALL");
        logger.warn("log");

        let (logger, _) = Logger::new("MAX");
        logger.warn("log");
    }

    #[test]
    fn test_error() {
        let (logger, _) = Logger::new("SILENT");
        logger.error("log");

        let (logger, _) = Logger::new("ERROR");
        logger.error("log");

        let (logger, _) = Logger::new("WARN");
        logger.error("log");

        let (logger, _) = Logger::new("INFO");
        logger.error("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.error("log");

        let (logger, _) = Logger::new("TRACE");
        logger.error("log");

        let (logger, _) = Logger::new("ALL");
        logger.error("log");

        let (logger, _) = Logger::new("MAX");
        logger.error("log");
    }

    #[test]
    fn test_info() {
        let (logger, _) = Logger::new("SILENT");
        logger.info("log");

        let (logger, _) = Logger::new("ERROR");
        logger.info("log");

        let (logger, _) = Logger::new("WARN");
        logger.info("log");

        let (logger, _) = Logger::new("INFO");
        logger.info("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.info("log");

        let (logger, _) = Logger::new("TRACE");
        logger.info("log");

        let (logger, _) = Logger::new("ALL");
        logger.info("log");

        let (logger, _) = Logger::new("MAX");
        logger.info("log");
    }

    #[test]
    fn test_success() {
        let (logger, _) = Logger::new("SILENT");
        logger.success("log");

        let (logger, _) = Logger::new("ERROR");
        logger.success("log");

        let (logger, _) = Logger::new("WARN");
        logger.success("log");

        let (logger, _) = Logger::new("INFO");
        logger.success("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.success("log");

        let (logger, _) = Logger::new("TRACE");
        logger.success("log");

        let (logger, _) = Logger::new("ALL");
        logger.success("log");

        let (logger, _) = Logger::new("MAX");
        logger.success("log");
    }

    #[test]
    fn test_debug() {
        let (logger, _) = Logger::new("SILENT");
        logger.debug("log");

        let (logger, _) = Logger::new("ERROR");
        logger.debug("log");

        let (logger, _) = Logger::new("WARN");
        logger.debug("log");

        let (logger, _) = Logger::new("INFO");
        logger.debug("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.debug("log");

        let (logger, _) = Logger::new("TRACE");
        logger.debug("log");

        let (logger, _) = Logger::new("ALL");
        logger.debug("log");

        let (logger, _) = Logger::new("MAX");
        logger.debug("log");
    }

    #[test]
    fn test_max() {
        let (logger, _) = Logger::new("SILENT");
        logger.debug_max("log");

        let (logger, _) = Logger::new("ERROR");
        logger.debug_max("log");

        let (logger, _) = Logger::new("WARN");
        logger.debug_max("log");

        let (logger, _) = Logger::new("INFO");
        logger.debug_max("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.debug_max("log");

        let (logger, _) = Logger::new("TRACE");
        logger.debug_max("log");

        let (logger, _) = Logger::new("ALL");
        logger.debug_max("log");

        let (logger, _) = Logger::new("MAX");
        logger.debug_max("log");
    }
}
