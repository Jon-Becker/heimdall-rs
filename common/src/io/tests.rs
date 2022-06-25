#[cfg(test)]
mod tests {
    use crate::io::logging::*;


    #[test]
    fn test_trace() {
        let logger = Logger::new("TRACE");
        let mut trace_factory = logger.trace;
        
        // indentation here matches the heirarchy of the trace
        let parent = trace_factory.add_trace("call", 0, 123123, vec!["Test::test_trace()".to_string()]);
                    
        trace_factory.add_trace("log", parent, 234234, vec!["ContractCreated(contractAddress: 0x0000000000000000000000000000000000000000)".to_string()]);
        
        let inner = trace_factory.add_trace("create", parent, 121234, vec!["TestContract".to_string(), "0x0000000000000000000000000000000000000000".to_string(), "917".to_string()]);
                
        trace_factory.add_trace("log_unknown", inner, 12344, vec!["0x0000000000000000000000000000000000000000000000000000000000000000".to_string()]);
        
        let deeper = trace_factory.add_trace("call", inner, 12344, vec!["Test::transfer(to: 0x0000000000000000000000000000000000000000, amount: 1)".to_string(), "true".to_string()]);
        trace_factory.add_trace("log", deeper, 12344, vec!["Transfer(from: 0x0000000000000000000000000000000000000000, to: 0x0000000000000000000000000000000000000000, amount: 1)".to_string()]);
        trace_factory.add_trace("info", inner, 12344, vec!["warn: Transfer to the zero address!".to_string()]);
        trace_factory.add_trace("info", parent, 12344, vec!["Execution Reverted: Out of Gas.".to_string(), "Execution Reverted: Out of Gas.".to_string()]);

        trace_factory.display()
    }

}