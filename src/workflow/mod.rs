/// Basic workflow state machine for orchestrating multi-step processes.

#[derive(Debug, PartialEq)]
pub enum WorkflowState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

pub struct WorkflowStep {
    pub name: String,
    pub handler: Box<dyn Fn() -> Result<(), String>>,
}

pub struct Workflow {
    pub id: String,
    pub state: WorkflowState,
    pub steps: Vec<WorkflowStep>,
    pub current: usize,
}

impl Workflow {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            state: WorkflowState::Pending,
            steps: Vec::new(),
            current: 0,
        }
    }

    pub fn add_step(&mut self, name: &str, handler: Box<dyn Fn() -> Result<(), String>>) {
        self.steps.push(WorkflowStep {
            name: name.to_string(),
            handler,
        });
    }

    pub fn run(&mut self) -> Result<(), String> {
        self.state = WorkflowState::Running;
        while self.current < self.steps.len() {
            if let Err(e) = self.steps[self.current].handler.as_ref()() {
                self.state = WorkflowState::Failed;
                return Err(format!(
                    "Step '{}' failed: {}",
                    self.steps[self.current].name, e
                ));
            }
            self.current += 1;
        }
        self.state = WorkflowState::Completed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_success() {
        let mut wf = Workflow::new("deploy");
        wf.add_step("init", Box::new(|| Ok(())));
        wf.add_step("execute", Box::new(|| Ok(())));
        
        assert_eq!(wf.state, WorkflowState::Pending);
        let res = wf.run();
        assert!(res.is_ok());
        assert_eq!(wf.state, WorkflowState::Completed);
    }

    #[test]
    fn test_workflow_failure() {
        let mut wf = Workflow::new("deploy");
        wf.add_step("init", Box::new(|| Ok(())));
        wf.add_step("fail", Box::new(|| Err("Boom".into())));
        wf.add_step("final", Box::new(|| Ok(())));
        
        let res = wf.run();
        assert!(res.is_err());
        assert_eq!(wf.state, WorkflowState::Failed);
        assert_eq!(wf.current, 1); // Failed at index 1
    }
}
