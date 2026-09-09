use op_agent::{
    AgentEvent, ApprovalDecision, ApprovalId, Budget, Effects, ItemId, Protocol, Session, TurnId,
    TurnStop, Usage, driver,
};
use serde_json::{Value, json};
use tokio::process::Command;

// Speaks to `cat`: every prompt goes out as one line and comes back as one message.
#[derive(Default)]
struct Echo {
    usage: Usage,
}

impl Protocol for Echo {
    type Input = Value;
    type Output = Value;

    fn read(&mut self, output: Value, effects: &mut Effects<Value>) {
        self.usage.output_tokens += 1;
        effects.emit(AgentEvent::Message {
            item: ItemId("echo".to_owned()),
            delta: output["text"].as_str().unwrap_or_default().to_owned(),
        });
    }

    fn prompt(&mut self, text: String, effects: &mut Effects<Value>) {
        effects.emit(AgentEvent::TurnStarted {
            turn: TurnId("1".to_owned()),
            prompt: text.clone(),
        });
        effects.send(json!({ "text": text }));
    }

    fn interrupt(&mut self, effects: &mut Effects<Value>) {
        effects.emit(AgentEvent::TurnEnded {
            turn: TurnId("1".to_owned()),
            stop: TurnStop::Interrupted,
        });
    }

    fn approve(&mut self, id: ApprovalId, _: ApprovalDecision, effects: &mut Effects<Value>) {
        effects.emit(AgentEvent::ApprovalResolved { id });
    }

    fn usage(&self) -> Usage {
        self.usage
    }
}

fn cat(budget: Budget) -> Session {
    driver::start(Echo::default(), Command::new("cat"), budget).expect("cat is on the path")
}

async fn next(session: &mut Session) -> AgentEvent {
    session
        .next_event()
        .await
        .expect("the driver is still running")
}

#[tokio::test]
async fn carries_a_prompt_to_the_process_and_its_answer_back() {
    let mut session = cat(Budget::default());
    session.handle().prompt("hello").await.unwrap();
    assert!(matches!(
        next(&mut session).await,
        AgentEvent::TurnStarted { prompt, .. } if prompt == "hello"
    ));
    assert!(matches!(
        next(&mut session).await,
        AgentEvent::Message { delta, .. } if delta == "hello"
    ));

    session.handle().shutdown().await.unwrap();
    assert_eq!(
        next(&mut session).await,
        AgentEvent::Exited { code: Some(0) }
    );
    assert_eq!(session.next_event().await, None);
}

#[tokio::test]
async fn stops_the_turn_when_the_budget_is_spent() {
    let mut session = cat(Budget::default().max_tokens(1));
    session.handle().prompt("one").await.unwrap();
    next(&mut session).await;
    next(&mut session).await;
    assert!(matches!(
        next(&mut session).await,
        AgentEvent::BudgetExhausted { session } if session.output_tokens == 1
    ));
    assert!(matches!(
        next(&mut session).await,
        AgentEvent::TurnEnded {
            stop: TurnStop::Interrupted,
            ..
        }
    ));

    session.handle().prompt("two").await.unwrap();
    assert!(matches!(
        next(&mut session).await,
        AgentEvent::Failed { message, retrying: false } if message.contains("budget")
    ));
}

#[tokio::test]
async fn ends_the_process_when_the_last_handle_is_dropped() {
    let mut session = cat(Budget::default());
    let (handle, mut events) = session.split();
    drop(handle);
    assert_eq!(
        events.recv().await,
        Some(AgentEvent::Exited { code: Some(0) })
    );
    assert_eq!(events.recv().await, None);
    session = cat(Budget::default());
    drop(session);
}

// A CLI that ignores a closed stdin still has to go, or a cancelled draft would leave a process
// behind for every cancel.
#[tokio::test(start_paused = true)]
async fn kills_a_process_that_ignores_a_closed_stdin() {
    let mut command = Command::new("sleep");
    command.arg("600");
    let mut session =
        driver::start(Echo::default(), command, Budget::default()).expect("sleep is on the path");
    session.handle().shutdown().await.unwrap();
    assert_eq!(next(&mut session).await, AgentEvent::Exited { code: None });
}
