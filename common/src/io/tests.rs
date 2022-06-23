#[cfg(test)]
mod tests {
    use crate::io::logging::*;


    #[test]
    fn test_trace() {
        let logger = Logger::new("TRACE");
        let mut trace_factory = logger.trace;
        
        println!("{:#?}", trace_factory);

        let mut trace = trace_factory.add_trace(0, vec!["test".to_string()]);


        let mut child1 = trace.add_child_trace(1, vec!["child1".to_string()]);
        let mut child2 = child1.add_child_trace(1, vec!["child2".to_string()]);
        let mut child3 = child2.add_child_trace(1, vec!["child3".to_string()]);
        let mut child4 = child3.add_child_trace(1, vec!["child4".to_string()]);
        let mut child5 = child3.add_child_trace(1, vec!["child5".to_string()]);
        println!("{:#?}", trace);
    }

}

// adding traces is simple
// 1. make a new tracefactory
// 2. make a new trace 
// 3. use trace.add_child_trace() to add children to the main trace etc
// 4. when the trace is complete, add it to the tracefactory
// 5. profit