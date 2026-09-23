import ./LAWS.bend as Laws

def main() -> {Laws.verdict_auto(Laws.Shadow{}) == Laws.Hold{} : Laws.Auto} & {Laws.transport(Laws.Denied{}) == Laws.Silent{} : Laws.Transport} & {Laws.approval(Laws.Empty{}) == Laws.No{} : Laws.Approval} & {Laws.state(Laws.Secret{}) == Laws.Rejected{} : Laws.State}:
  (
    Laws.shadow_cannot_set_auto_allow(),
    (
      Laws.code_deny_never_calls_transport(),
      (
        Laws.empty_findings_are_not_approval(),
        Laws.state_rejects_secret_key()
      )
    )
  )
