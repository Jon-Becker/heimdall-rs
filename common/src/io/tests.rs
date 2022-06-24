#[cfg(test)]
mod tests {
    use crate::io::logging::*;


    #[test]
    fn test_trace() {
        let logger = Logger::new("TRACE");
        let mut trace_factory = logger.trace;
        
        let parent = trace_factory.add_trace(0, 123123, vec![]);
        trace_factory.add_trace(parent, 234234, vec!["child".to_string()]);
        let child = trace_factory.add_trace(parent, 121234, vec!["child".to_string()]);
        trace_factory.add_trace(child, 999999, vec!["test".to_string(), "balls".to_string()]);
        let child = trace_factory.add_trace(parent, 12344, vec!["child".to_string()]);


        println!("{:#?}", trace_factory);
        trace_factory.print()
    }

}

//0
//1
//1
//2
//1