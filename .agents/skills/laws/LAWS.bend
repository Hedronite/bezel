import Base

type Mode is Data:
  Shadow{}
  Active{}

type Auto is Data:
  Hold{}
  Allow{}

type Call is Data:
  Denied{}
  Open{}

type Transport is Data:
  Silent{}
  Called{}

type Findings is Data:
  Empty{}
  Present{}

type Approval is Data:
  No{}
  Yes{}

type Key is Data:
  Public{}
  Secret{}

type State is Data:
  Rejected{}
  Seated{}

def verdict_auto(mode: Mode) -> Auto:
  match mode:
    case Shadow{}:
      Hold{}
    case Active{}:
      Hold{}

def transport(call: Call) -> Transport:
  match call:
    case Denied{}:
      Silent{}
    case Open{}:
      Called{}

def approval(findings: Findings) -> Approval:
  match findings:
    case Empty{}:
      No{}
    case Present{}:
      Yes{}

def state(key: Key) -> State:
  match key:
    case Secret{}:
      Rejected{}
    case Public{}:
      Seated{}

law shadow_cannot_set_auto_allow:
  {verdict_auto(Shadow{}) == Hold{} : Auto}

def shadow_cannot_set_auto_allow():
  {==}

law code_deny_never_calls_transport:
  {transport(Denied{}) == Silent{} : Transport}

def code_deny_never_calls_transport():
  {==}

law empty_findings_are_not_approval:
  {approval(Empty{}) == No{} : Approval}

def empty_findings_are_not_approval():
  {==}

law state_rejects_secret_key:
  {state(Secret{}) == Rejected{} : State}

def state_rejects_secret_key():
  {==}
