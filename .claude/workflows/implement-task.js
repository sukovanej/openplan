export const meta = {
  name: 'implement-task',
  description: 'Implement an OPP task test-first: red tests, reviewed, then implement/review to green and a PR',
  whenToUse:
    'When a task in .plan/tasks/ is ready to build. Pass args {taskKey:"OPP-42", maxRounds?:3, base?:"main"}. ' +
    'Writes and reviews tests before any implementation. Returns a PR URL; leaves the task in_review.',
  phases: [
    { title: 'Setup', detail: 'worktree, web dist, mark in_progress' },
    { title: 'Plan', detail: 'task body to file-scoped code and test units' },
    { title: 'Stubs', detail: 'minimal todo!() surface so tests can compile' },
    { title: 'Tests', detail: 'one agent per test file, no implementation' },
    { title: 'Red gate', detail: 'builds, and every new test fails for the right reason' },
    { title: 'Test review', detail: 'coverage, e2e altitude, useless tests' },
    { title: 'Test verify', detail: 'majority vote per test finding' },
    { title: 'Implement', detail: 'one agent per file, tests are frozen' },
    { title: 'Checks', detail: 'build, test, fmt, clippy — retried until green' },
    { title: 'Review', detail: 'parallel dimensions over the diff' },
    { title: 'Verify', detail: 'majority vote per code finding' },
    { title: 'Finalize', detail: 'in_review, commit, push, gh pr create' },
  ],
}

async function main() {
  const task = parseTaskArgs()

  phase('Setup')
  const ctx = { ...(await run(setupStep(task))), key: task.key, base: task.base }
  log(`${ctx.key} — ${ctx.title} → ${ctx.worktree}`)

  phase('Plan')
  const plan = await run(planStep(ctx))
  if (!plan.coversTask) return halt(ctx, 'plan-mismatch', { mismatch: plan.mismatch })
  log(`${plan.tests.length} test unit(s), ${plan.units.length} production unit(s)`)

  const tests = plan.tests.length ? await testFirst(ctx, plan, task.maxRounds) : NO_TESTS
  if (tests.halted) return halt(ctx, tests.halted, { failures: tests.gate.failures, unresolved: tests.unresolved })

  const code = await refineLoop(ctx, CODE_STAGE, plan.units, (r, a) => greenGateStep(ctx, r, a), task.maxRounds)

  return openPullRequest(ctx, tests, code)
}

async function testFirst(ctx, plan, maxRounds) {
  phase('Stubs')
  const stubs = await run(stubsStep(ctx, plan))
  if (!stubs.passed) return { halted: 'stubs-do-not-compile', gate: stubs, unresolved: [] }

  const tests = await refineLoop(ctx, TEST_STAGE, plan.tests, (r, a) => redGateStep(ctx, plan, r, a), maxRounds)
  if (!tests.gate.passed) return { ...tests, halted: 'no-clean-red' }

  log(`Tests are red for the right reasons after ${tests.rounds} round(s) — implementing`)
  return tests
}

async function refineLoop(ctx, stage, initialWork, gateFor, maxRounds) {
  const seen = new Set()
  const history = []
  let work = initialWork
  let unresolved = []
  let gate = null
  let converged = false
  let round = 0

  while (round < maxRounds) {
    round += 1

    gate = await applyUntilGate(ctx, work, stage, gateFor, round)
    if (!gate.passed) {
      log(`${stage.short}: abandoning after ${GATE_ATTEMPTS} attempts — the gate never went green`)
      break
    }

    const found = await reviewDiff(ctx, stage, round)
    const fresh = found.filter(f => !seen.has(findingKey(f)))
    fresh.forEach(f => seen.add(findingKey(f)))
    log(`${stage.short} round ${round}: ${found.length} findings, ${fresh.length} new`)
    if (!fresh.length) {
      converged = true
      break
    }

    const confirmed = await verifyAll(ctx, stage, fresh)
    log(`${stage.short} round ${round}: ${confirmed.length}/${fresh.length} survived verification`)
    history.push({ stage: stage.short, round, found: fresh.length, confirmed: confirmed.length })
    if (!confirmed.length) {
      converged = true
      break
    }

    unresolved = confirmed
    work = findingsToWork(confirmed)
  }

  return { converged, gate, unresolved, rounds: round, history }
}

async function applyUntilGate(ctx, initialWork, stage, gateFor, round) {
  let work = initialWork
  let gate = null

  for (let attempt = 1; attempt <= GATE_ATTEMPTS; attempt++) {
    phase(stage.phases.apply)
    await parallel(work.map(u => () => run(applyStep(ctx, u, stage, round, attempt))))

    gate = await run(gateFor(round, attempt))
    if (gate.passed) return gate

    log(`${stage.short} round ${round}: gate red, ${gate.failures.length} failure(s), attempt ${attempt}/${GATE_ATTEMPTS}`)
    work = failuresToWork(gate, stage.short)
  }

  return gate
}

async function reviewDiff(ctx, stage, round) {
  phase(stage.phases.review)
  const dimensions = round === 1 ? stage.dimensions : stage.dimensions.filter(d => d.rerun)
  const perDimension = await parallel(
    dimensions.map(d => () =>
      run(reviewStep(ctx, d, stage, round)).then(r =>
        (r.findings || []).map(f => ({ ...f, dimension: d.key, angles: d.angles, question: d.question })),
      ),
    ),
  )
  return perDimension.filter(Boolean).flat()
}

async function verifyAll(ctx, stage, findings) {
  phase(stage.phases.verify)
  const verified = await parallel(
    findings.map(f => () =>
      parallel(f.angles.map(angle => () => run(verifyStep(ctx, f, angle, stage)))).then(votes => ({
        finding: f,
        keeps: votes.filter(Boolean).filter(v => v.keep).length,
      })),
    ),
  )
  return verified
    .filter(Boolean)
    .filter(v => v.keeps > v.finding.angles.length / 2)
    .map(v => v.finding)
}

async function openPullRequest(ctx, tests, code) {
  const checksPassed = !!(code.gate && code.gate.passed)
  const converged = tests.converged && code.converged
  const unresolved = [...tests.unresolved, ...code.unresolved]
  const outcome = {
    converged,
    checksPassed,
    failures: code.gate ? code.gate.failures : [],
    unresolved,
    draft: !converged || !checksPassed,
  }
  if (!checksPassed) log(`Handing off red: ${outcome.failures.length} failure(s) from \`${CHECKS}\``)
  else if (!converged) log(`Handing off with ${unresolved.length} finding(s) left open`)

  phase('Finalize')
  const pr = await run(finalizeStep(ctx, outcome))
  log(`${outcome.draft ? 'Draft PR' : 'PR'} ready: ${pr.url}`)

  return {
    taskKey: ctx.key,
    title: ctx.title,
    worktree: ctx.worktree,
    branch: ctx.branch,
    pr: pr.url,
    draft: outcome.draft,
    converged,
    checksPassed,
    testRounds: tests.rounds,
    codeRounds: code.rounds,
    unresolved: unresolved.map(f => ({ dimension: f.dimension, file: f.file, line: f.line, summary: f.summary })),
    history: [...tests.history, ...code.history],
  }
}

const run = ({ prompt, ...opts }) => agent(prompt, opts)

const halt = (ctx, reason, extra) => ({
  taskKey: ctx.key,
  worktree: ctx.worktree,
  halted: reason,
  pr: null,
  ...extra,
})

const NO_TESTS = { converged: true, unresolved: [], rounds: 0, history: [] }

function parseTaskArgs() {
  const key = typeof args === 'string' ? args : args && args.taskKey
  if (!key || !/^[A-Z]{3}-[1-9][0-9]*$/.test(key)) {
    throw new Error(`args.taskKey must be a task key like "OPP-42", got ${JSON.stringify(key)}`)
  }
  return { key, base: (args && args.base) || 'main', maxRounds: (args && args.maxRounds) || 3 }
}

function findingKey(f) {
  const claim = String(f.summary).toLowerCase().replace(/[^a-z0-9]+/g, '').slice(0, 60)
  return `${f.file}:${f.line}:${claim}`
}

function failuresToWork(gate, label) {
  return gate.failures.map(f => ({
    file: f.file,
    instruction: `Fix this ${label} failure. Change only what the failure requires:\n\n${f.error}`,
  }))
}

function findingsToWork(findings) {
  const byFile = new Map()
  for (const f of findings) {
    if (!byFile.has(f.file)) byFile.set(f.file, [])
    byFile.get(f.file).push(f)
  }
  return [...byFile.entries()].map(([file, fs]) => ({
    file,
    instruction: `Apply these ${fs.length} confirmed review finding(s). Do not refactor beyond them:\n${fs
      .map(f => `- [${f.dimension}] ${f.summary} (${f.file}:${f.line}) — ${f.failureScenario}`)
      .join('\n')}`,
  }))
}

const CHECKS = 'cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings'
const GATE_ATTEMPTS = 3

const str = { type: 'string' }
const bool = { type: 'boolean' }
const num = { type: 'number' }
const list = items => ({ type: 'array', items })
const rec = (required, properties) => ({ type: 'object', required, properties })

const SCHEMA = {
  setup: rec(['worktree', 'branch', 'title', 'body', 'verifyCriteria', 'touchesWeb', 'touchesApi'], {
    worktree: str,
    branch: str,
    title: str,
    body: str,
    verifyCriteria: list(str),
    touchesWeb: bool,
    touchesApi: bool,
  }),
  plan: rec(['units', 'tests', 'coversTask', 'mismatch'], {
    coversTask: bool,
    mismatch: str,
    units: list(rec(['file', 'instruction'], { file: str, instruction: str })),
    tests: list(rec(['file', 'instruction', 'criteria'], { file: str, instruction: str, criteria: list(str) })),
  }),
  gate: rec(['passed', 'failures'], {
    passed: bool,
    failures: list(rec(['file', 'error'], { file: str, error: str })),
  }),
  findings: rec(['findings'], {
    findings: list(
      rec(['file', 'line', 'summary', 'failureScenario'], {
        file: str,
        line: num,
        summary: str,
        failureScenario: str,
      }),
    ),
  }),
  verdict: rec(['keep', 'reasoning'], { keep: bool, reasoning: str }),
  pr: rec(['url'], { url: str, sha: str }),
}

const DEMONSTRATE = `Read the real code and its callers, then decide whether the failure actually
occurs. keep=false unless you can demonstrate it.`

const WORTH_CHURN = `Decide whether this is worth changing in a diff that already passes \`${CHECKS}\`.
Reject nits and taste-only rewrites. keep=true only if leaving it would make the codebase measurably
worse against CLAUDE.md or the task's own criteria.`

const CATCHES_REGRESSION = `Answer one question: if the behaviour this test claims to cover were
broken, would this test fail? keep=true only if the finding survives that question.`

const TEST_DIMENSIONS = [
  {
    key: 'criteria-coverage',
    rerun: true,
    angles: ['walk-the-verify-list', 'walk-the-surfaces-list'],
    question: DEMONSTRATE,
    brief: `Walk the task's Verify criteria one by one and name every one no test exercises. Flag
partial coverage too — a criterion listing four refusals needs all four. Report each gap against the
test file that should close it.`,
  },
  {
    key: 'test-quality',
    rerun: true,
    angles: ['would-it-catch-a-break', 'is-there-a-boundary-test'],
    question: CATCHES_REGRESSION,
    brief: `Flag tests that cannot fail for a real reason, in two forms.

Wrong altitude: asserting on internals when the behaviour is observable at a boundary — the CLI
(crates/op-cli/tests/cli.rs), the HTTP API (op-server), or a crate's public surface.

No teeth: tautologies, assertions on derives or plain getters, tests proving only that a fixture
returns its own configuration, and assertions weaker than the behaviour they name.`,
  },
]

const CODE_DIMENSIONS = [
  {
    key: 'task-fidelity',
    rerun: true,
    angles: ['against-the-verify-section', 'against-the-spec-changes', 'against-the-surfaces-list'],
    question: DEMONSTRATE,
    brief: `Report anything the task requires that is missing, half-done, or contradicted — including a
SPEC.md rewrite the task asks for and the diff does not make.`,
  },
  {
    key: 'correctness',
    rerun: true,
    angles: ['does-it-reproduce', 'read-the-callers', 'check-the-types'],
    question: DEMONSTRATE,
    brief: `Find logic errors, panics, unwraps on fallible paths, races, and wrong error handling the
test suite does not already catch. Give the exact inputs or state that produce the wrong result.`,
  },
  {
    key: 'duplication',
    rerun: true,
    angles: ['are-they-really-the-same', 'would-merging-help'],
    question: DEMONSTRATE,
    brief: `Find logic in the diff duplicating something already in the workspace; cite both sites.
Semantic duplication counts, similar-looking but differently-purposed code does not.`,
  },
  {
    key: 'coverage-drift',
    rerun: false,
    angles: ['worth-the-churn'],
    question: WORTH_CHURN,
    brief: `The tests were written before this code existed. Find behaviour the implementation
introduced that they do not reach — branches and error paths that appeared during implementation.`,
  },
  {
    key: 'organization',
    rerun: false,
    angles: ['worth-the-churn'],
    question: WORTH_CHURN,
    brief: `Find misplaced responsibilities and leaked abstractions. This workspace splits
op-task / op-store / op-api / op-cli / op-server by layer — flag code sitting in the wrong one.`,
  },
  {
    key: 'conventions',
    rerun: false,
    angles: ['worth-the-churn'],
    question: WORTH_CHURN,
    brief: `Check against CLAUDE.md: no doc comments, no comments restating what the code does, tests
never inside src/, README limited to build/run essentials. Flag prose that exists because the code is
unclear.`,
  },
]

const TEST_STAGE = {
  short: 'test',
  scope: 'tests',
  dimensions: TEST_DIMENSIONS,
  phases: { apply: 'Tests', review: 'Test review', verify: 'Test verify' },
}

const CODE_STAGE = {
  short: 'impl',
  scope: 'code',
  dimensions: CODE_DIMENSIONS,
  phases: { apply: 'Implement', review: 'Review', verify: 'Verify' },
}

const WRITE_SCOPE = {
  tests: `You are in the test-first stage. Write tests only; production code stays \`todo!()\`. If you
find yourself implementing behaviour to make a test pass, stop — a test that passes right now proves
nothing.`,
  code: `The tests are the specification and they are frozen. Do not weaken, delete, or rewrite a
test's expectations to make it pass; change the production code instead. Add a new test only when a
confirmed review finding asks for one.`,
}

function agentRules(ctx) {
  return `Work exclusively inside the worktree ${ctx.worktree} (branch ${ctx.branch}).

Rules that override your defaults:
- Every write goes inside that worktree. Never touch the primary checkout at the repo root.
- Do NOT run any cargo command. Cargo locks the shared target directory and you are running
  beside other agents; a single gate agent owns all cargo invocations.
- No doc comments (/// or //!). A plain comment is allowed only for *why* the code cannot
  express itself. Never restate what the code does.
- Tests live in the crate's tests/ directory. Never add #[cfg(test)] mod tests to src/.
- Recurring workflows are mise tasks; do not hand-run their underlying commands.`
}

function setupStep(task) {
  return {
    label: `setup:${task.key}`,
    schema: SCHEMA.setup,
    prompt: `Prepare an isolated worktree for task ${task.key} in the open-plan repo.

1. Read the task: \`oplan get ${task.key} --json\`. If the key does not resolve, fail loudly.
2. From the primary checkout, create ONE worktree for this whole task:
     \`git worktree add .claude/worktrees/<slug> -b task/<slug> ${task.base}\`
   where <slug> is a short kebab-case form of the task title.
3. Copy \`web/packages/app/dist\` from the primary checkout into the new worktree. It is gitignored
   and op-server embeds it (crates/op-server/src/lib.rs) — without it the op-server tests serve 404
   and fail for reasons unrelated to this task.
4. Then, as the FIRST write made inside the worktree:
     \`oplan set ${task.key} status in_progress\`
   Do not run this from the primary checkout.

Return the worktree path, branch, task title, full task body, the "## Verify" section as a list of
concrete criteria, and whether the task touches the web SPA and the HTTP/OpenAPI surface.`,
  }
}

function planStep(ctx) {
  return {
    label: `plan:${ctx.key}`,
    schema: SCHEMA.plan,
    effort: 'high',
    prompt: `${agentRules(ctx)}

Turn task ${ctx.key} into two file-scoped work lists: the tests to write first, and the production
files to change after. Read the whole task body and the files it names first. The task body is the
design — do not redesign it.

Task body:
${ctx.body}

Verify criteria:
${ctx.verifyCriteria.map(c => `- ${c}`).join('\n')}

"tests" — one unit per test file, carrying the Verify criteria it must cover and what behaviour to
assert at which boundary. Prefer the CLI (crates/op-cli/tests/cli.rs) and HTTP boundaries over crate
internals when the behaviour is observable there. Every criterion above must appear in some unit's
"criteria".

"units" — one unit per production file, including SPEC.md.

Never emit two units for the same file in either list; one agent owns one file. Leave "tests" empty
only for a genuinely untestable change such as docs-only.

Set coversTask=false and describe the gap in "mismatch" if the task is ambiguous, self-contradictory,
or asks for something you cannot locate. Do not guess.`,
  }
}

function stubsStep(ctx, plan) {
  return {
    label: `stubs:${ctx.key}`,
    schema: SCHEMA.gate,
    effort: 'high',
    prompt: `${agentRules(ctx)}

Task ${ctx.key} — "${ctx.title}". Lay the groundwork for test-first development.

Add the minimal API surface the planned tests will call — types, signatures, trait impls, CLI
subcommands, routes — with every body \`todo!()\`. No logic, no behaviour, no error handling.

The point is that \`cargo build\` succeeds so \`cargo test\` actually runs and fails at \`todo!()\`.
Without this a missing implementation is indistinguishable from a broken test, because neither
compiles.

Test files that will call it:
${plan.tests.map(t => `- ${t.file} — ${t.criteria.join('; ')}`).join('\n')}

Production files in scope:
${plan.units.map(u => `- ${u.file}`).join('\n')}

You are the only agent in this stage, so every signature must agree across files. You may run
\`cargo build\` — and only that — to confirm the surface compiles. Report anything you could not make
compile as a failure.`,
  }
}

function applyStep(ctx, unit, stage, round, attempt) {
  return {
    label: `${stage.short}:r${round}.${attempt}:${unit.file}`,
    phase: stage.phases.apply,
    prompt: `${agentRules(ctx)}

Task ${ctx.key} — "${ctx.title}".

${WRITE_SCOPE[stage.scope]}

Change ONLY ${unit.file}. Another agent owns every other file; do not edit, create, or move anything
else. Expect the workspace to be mid-change until they finish — that is not yours to fix, and it is
why you must not run cargo.

${unit.instruction}`,
  }
}

function redGateStep(ctx, plan, round, attempt) {
  return {
    label: `red:r${round}.${attempt}`,
    phase: 'Red gate',
    schema: SCHEMA.gate,
    prompt: `${agentRules(ctx)}

You are the only agent permitted to run cargo. Run \`cargo build\`, then \`cargo test\`. Do NOT run
fmt or clippy — a \`todo!()\` surface trips unused-variable warnings that say nothing about the tests.

This is the red gate of a TDD cycle: the tests exist, the implementation does not. Report a failure
ONLY for a wrong-reason outcome:
- \`cargo build\` fails — a test or stub is malformed; attribute it to that file.
- a new test PASSES — it does not exercise unimplemented behaviour, so it is tautological or asserts
  nothing. This is the most important one to catch.
- a new test fails before reaching the behaviour — wrong import, bad fixture, missing test data.

Expected and NOT a failure: a new test failing at a \`todo!()\` panic, or on an assertion about
behaviour that does not exist yet.

The new test files:
${plan.tests.map(t => `- ${t.file}`).join('\n')}

Fix nothing. passed=true when the workspace compiles and every new test fails for the expected
reason.`,
  }
}

function greenGateStep(ctx, round, attempt) {
  const generate = ctx.touchesApi
    ? '1. `mise run generate-web-client` — the HTTP surface changed, so the committed Effect client must be regenerated.\n'
    : ''
  const web = ctx.touchesWeb ? '3. `pnpm -C web -r build`, plus the web test suite.\n' : ''
  return {
    label: `checks:r${round}.${attempt}`,
    phase: 'Checks',
    schema: SCHEMA.gate,
    prompt: `${agentRules(ctx)}

You are the only agent permitted to run cargo. Run these in order, reporting the first failure of
each:

${generate}2. \`${CHECKS}\`
${web}
Do NOT run \`mise run rebuild\` — it restarts the user's daemon, a side effect outside this task's
scope. Build the SPA directly instead.

Every test from the test-first stage must now pass, and no \`todo!()\` may remain on a path the task
requires. A still-failing test means the implementation is incomplete: report it against the
production file that should satisfy it, never against the test.

Fix nothing. Report each failure with its file and the compiler or test output verbatim. passed=true
only if every command above exited zero.`,
  }
}

function reviewStep(ctx, dimension, stage, round) {
  return {
    label: `${stage.short}-review:r${round}:${dimension.key}`,
    phase: stage.phases.review,
    schema: SCHEMA.findings,
    prompt: `${agentRules(ctx)}

Review the diff of branch ${ctx.branch} against ${ctx.base} for task ${ctx.key}.

${dimension.brief}

Task body:
${ctx.body}

Verify criteria:
${ctx.verifyCriteria.map(c => `- ${c}`).join('\n')}

Read the actual code before reporting anything. Report only findings you can point at with
file:line. An empty findings array is a valid and useful answer.`,
  }
}

function verifyStep(ctx, finding, angle, stage) {
  return {
    label: `verify:${angle}:${finding.file}`,
    phase: stage.phases.verify,
    schema: SCHEMA.verdict,
    effort: finding.angles.length > 1 ? 'high' : 'low',
    prompt: `${agentRules(ctx)}

Judge this ${finding.dimension} finding through the "${angle}" lens:

  ${finding.summary}
  at ${finding.file}:${finding.line}
  claimed failure: ${finding.failureScenario}

${finding.question}`,
  }
}

function finalizeStep(ctx, outcome) {
  const checksLine = outcome.checksPassed
    ? `a line confirming \`${CHECKS}\` passes`
    : `a "Checks red" section quoting the ${outcome.failures.length} outstanding failure(s)`
  const findingsLine = outcome.converged
    ? 'nothing about review findings'
    : `an "Unresolved review findings" section listing:\n${outcome.unresolved
        .map(f => `     - [${f.dimension}] ${f.summary} (${f.file}:${f.line})`)
        .join('\n')}`
  return {
    label: `pr:${ctx.key}`,
    schema: SCHEMA.pr,
    prompt: `${agentRules(ctx)}

Task ${ctx.key} — "${ctx.title}" is ready for human review.

1. \`oplan set ${ctx.key} status in_review\`. Never set it to done; a human does that.
2. Stage everything in the worktree, including the .plan/tasks/ status change, and commit:

     ${ctx.title}

     <a short body: what changed and why, not a restatement of the diff>

     Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

3. Push the branch and open a PR against ${ctx.base} with
   \`gh pr create${outcome.draft ? ' --draft' : ''}\`, titled "${ctx.title}".

The PR body must contain, in this order:
   - one paragraph on the change
   - "Closes ${ctx.key}"
   - the Verify criteria as a checklist, each ticked with the test covering it — those tests were
     written before the implementation, so name them
   - ${checksLine}
   - ${findingsLine}
   - the footer: 🤖 Generated with [Claude Code](https://claude.com/claude-code)

Return the PR URL and the head commit sha.`,
  }
}

return main()
