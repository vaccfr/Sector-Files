const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { parseCycles, gateResult, evaluate, syncLabels, tagCycle } = require('./airac.cjs');

const cycles = parseCycles('2026-10-29: "2611"\n2026-11-26: "2612"\n');
const before = new Date('2026-10-28T23:59:59Z');
test('scheduled refreshes do not create duplicate commit statuses', async () => {
  const f = fixture([pull(['airac/2611'])], [], [], [{
    context: 'AIRAC merge gate', state: 'pending',
    description: 'Blocked until 2026-10-29 at 00:00 UTC',
  }]);
  await evaluate({ ...f, cycles, now: before });
  assert.equal(f.calls.filter(([name]) => name === 'status').length, 0);
  await evaluate({ ...f, cycles, now: effective });
  assert.equal(f.calls.find(([name]) => name === 'status')[1].state, 'success');
});

test('an unreadable calendar fails closed for every open PR', async t => {
  const f = fixture([pull(), { ...pull(['airac/2611']), number: 2 }]);
  t.mock.method(fs, 'readFileSync', () => { throw new Error('broken calendar'); });
  await evaluate({ ...f, now: before });
  const statuses = f.calls.filter(([name]) => name === 'status');
  assert.equal(statuses.length, 2);
  assert.ok(statuses.every(([, value]) => value.state === 'failure'));
  assert.equal(f.calls.filter(([name]) => name === 'failure').length, 1);
});

const effective = new Date('2026-10-29T00:00:00Z');
const pull = (labels = [], body = '') => ({
  number: 1, state: 'open', base: { ref: 'main' }, head: { sha: 'latest-sha' }, labels, body,
});

function fixture(prs = [], labels = [], comments = [], statuses = []) {
  const calls = [];
  const endpoint = (name, data) => async args => { calls.push([name, args]); return { data }; };
  const rest = {
    pulls: { list: 'pulls.list', get: async ({ pull_number }) => ({ data: prs.find(pr => pr.number === pull_number) }) },
    repos: { createCommitStatus: endpoint('status'), listCommitStatusesForRef: 'statuses.list' },
    issues: {
      listComments: 'issues.listComments', listLabelsForRepo: 'issues.listLabelsForRepo',
      createComment: endpoint('createComment'), updateComment: endpoint('updateComment'),
      createLabel: endpoint('createLabel'), updateLabel: endpoint('updateLabel'), deleteLabel: endpoint('deleteLabel'),
    },
    git: {
      getRef: async args => {
        calls.push(['getRef', args]);
        if (args.ref.startsWith('tags/')) throw Object.assign(new Error('Not found'), { status: 404 });
        return { data: { object: { sha: 'main-sha' } } };
      },
      createTag: endpoint('createTag', { sha: 'tag-sha' }), createRef: endpoint('createRef'),
    },
  };
  return {
    calls, context: { repo: { owner: 'vaccfr', repo: 'Sector-Files' }, payload: {} },
    core: { info() {}, setFailed: message => calls.push(['failure', message]) },
    github: { rest, paginate: async endpoint => {
      if (endpoint === rest.repos.listCommitStatusesForRef) return statuses;
      if (endpoint === rest.pulls.list) return prs;
      if (endpoint === rest.issues.listLabelsForRepo) return labels;
      if (endpoint === rest.issues.listComments) return comments;
      throw new Error('Unexpected endpoint');
    } },
  };
}

test('calendar rejects impossible dates, duplicates, malformed and empty input', () => {
  for (const text of ['', '2026-02-30: "2602"', '2026-10-29: 2611', 'bad',
    '2026-10-29: "2611"\n2026-11-26: "2611"', '2026-10-29: "2611"\n2026-10-29: "2612"']) {
    assert.throws(() => parseCycles(text));
  }
});

test('shipped calendar contains 39 unique, consecutive 28-day cycles', () => {
  const calendar = parseCycles(fs.readFileSync('.github/airac-cycles.yml', 'utf8'));
  assert.equal(calendar.size, 39);
  const dates = [...calendar.values()];
  for (let i = 1; i < dates.length; i++) assert.equal(Date.parse(dates[i]) - Date.parse(dates[i - 1]), 28 * 86400000);
});

test('gate opens exactly at midnight UTC and handles unlabelled PRs', () => {
  assert.equal(gateResult(pull(), cycles, before).state, 'success');
  assert.equal(gateResult(pull(['airac/2611']), cycles, before).state, 'pending');
  assert.equal(gateResult(pull([{ name: 'airac/2611' }]), cycles, effective).state, 'success');
});

test('all cycle labels apply; unknown labels fail even alongside valid labels', () => {
  assert.equal(gateResult(pull(['airac/2612', 'airac/2611']), cycles, effective).date, '2026-11-26');
  assert.equal(gateResult(pull(['airac/2611', 'airac/9999']), cycles, before).state, 'failure');
  assert.equal(gateResult(pull(['airac/not-a-cycle']), cycles, before).state, 'failure');
});

test('old date-labelled PRs remain protected during migration', () => {
  const body = 'AIRAC effective date: ' + String.fromCharCode(96) + '2026-11-26' + String.fromCharCode(96);
  assert.equal(gateResult(pull(['airac-dependent'], body), cycles, effective).state, 'pending');
  assert.equal(gateResult(pull(['airac-dependent'], 'AIRAC effective date: 2026-02-30'), cycles, before).state, 'failure');
  assert.equal(gateResult(pull(['airac-dependent']), cycles, before).state, 'failure');
});

test('gate refreshes PR head and labels rather than using stale webhook fields', async () => {
  const f = fixture([pull(['airac/2612'])]);
  f.context.payload.pull_request = { ...pull(), head: { sha: 'stale-sha' } };
  await evaluate({ ...f, cycles, now: effective });
  const status = f.calls.find(([name]) => name === 'status')[1];
  assert.equal(status.sha, 'latest-sha');
  assert.equal(status.state, 'pending');
  assert.equal(status.context, 'AIRAC merge gate');
  assert.equal(f.calls.filter(([name]) => name === 'createComment').length, 1);
});

test('unchanged bot comments are reused; unlabelling updates the existing comment', async () => {
  const f = fixture([pull(['airac/2611'])]);
  await evaluate({ ...f, cycles, now: before });
  const body = f.calls.find(([name]) => name === 'createComment')[1].body;
  const comments = [{ id: 42, user: { login: 'github-actions[bot]' }, body }];
  const same = fixture([pull(['airac/2611'])], [], comments);
  await evaluate({ ...same, cycles, now: before });
  assert.equal(same.calls.filter(([name]) => /Comment$/.test(name)).length, 0);
  const removed = fixture([pull()], [], comments);
  removed.context.payload = { action: 'unlabeled', pull_request: pull() };
  await evaluate({ ...removed, cycles, now: before });
  assert.match(removed.calls.find(([name]) => name === 'updateComment')[1].body, /No AIRAC dependency/);
});

test('closed PRs and other target branches are ignored; errors do not skip remaining PRs', async () => {
  const closed = { ...pull(), state: 'closed' };
  const other = { ...pull(), number: 2, base: { ref: 'other' } };
  const live = { ...pull(), number: 3 };
  const f = fixture([closed, other, live]);
  await evaluate({ ...f, cycles, now: before });
  assert.equal(f.calls.filter(([name]) => name === 'status').length, 1);
  const g = fixture([pull(), { ...pull(), number: 2 }]);
  const original = g.github.rest.pulls.get;
  g.github.rest.pulls.get = args => {
    if (args.pull_number === 1) throw new Error('API failure');
    return original(args);
  };
  await evaluate({ ...g, cycles, now: before });
  assert.equal(g.calls.filter(([name]) => name === 'failure').length, 1);
  assert.equal(g.calls.filter(([name]) => name === 'status').length, 1);
});

test('label sync creates the upcoming window and preserves labels on any open PR', async () => {
  const calendar = new Map([['2610', '2026-10-01'], ...cycles, ['2613', '2026-12-24']]);
  const labels = ['airac/2610', 'airac/2613', 'custom'].map(name => ({ name, color: 'DC521D' }));
  const f = fixture([{ ...pull(['airac/2613']), base: { ref: 'other' } }], labels);
  await syncLabels({ ...f, cycles: calendar, now: before, maxUpcoming: 2 });
  assert.deepEqual(f.calls.filter(([name]) => name === 'createLabel').map(([, args]) => args.name),
    ['airac/2611', 'airac/2612']);
  assert.deepEqual(f.calls.filter(([name]) => name === 'deleteLabel').map(([, args]) => args.name), ['airac/2610']);
  await assert.rejects(syncLabels({ ...f, cycles, maxUpcoming: 0 }), /max_upcoming/);
  await assert.rejects(syncLabels({ ...f, cycles, now: new Date('2030-01-01') }), /No upcoming/);
});

test('tagging snapshots main on effective days and leaves existing tags unchanged', async () => {
  const f = fixture();
  await tagCycle({ ...f, cycles, now: effective });
  assert.equal(f.calls.find(([name]) => name === 'createTag')[1].object, 'main-sha');
  assert.equal(f.calls.find(([name]) => name === 'createRef')[1].ref, 'refs/tags/airac-2611');
  const existing = fixture();
  existing.github.rest.git.getRef = async () => ({ data: {} });
  await tagCycle({ ...existing, cycles, now: effective });
  assert.equal(existing.calls.length, 0);
  const ordinary = fixture();
  await tagCycle({ ...ordinary, cycles, now: before });
  assert.equal(ordinary.calls.length, 0);
});

test('VACS-style comments show the governing cycle, auto-merge guidance and related PR link', async () => {
  const pending = fixture([pull(['airac/2611', 'airac/2612'])]);
  await evaluate({ ...pending, cycles, now: before });
  const blocked = pending.calls.find(([name]) => name === 'createComment')[1].body;
  assert.match(blocked, /\*\*Merge blocked\*\*/);
  assert.match(blocked, /AIRAC cycle 2612/);
  assert.match(blocked, /2026-11-26 at 00:00 UTC/);
  assert.match(blocked, /enable auto-merge/);
  assert.match(blocked, /all required reviews and checks pass/);
  assert.ok(blocked.includes('https://github.com/vaccfr/Sector-Files/pulls?q='
    + encodeURIComponent('is:pr is:open label:airac/2612')));
  const ready = fixture([pull(['airac/2611'])]);
  await evaluate({ ...ready, cycles, now: effective });
  const unlocked = ready.calls.find(([name]) => name === 'createComment')[1].body;
  assert.ok(unlocked.includes('> [!TIP]'));
  assert.match(unlocked, /AIRAC cycle 2611 is now effective/);
  assert.match(unlocked, /If auto-merge is enabled/);
});
