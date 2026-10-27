const fs = require('node:fs');

const STATUS = 'AIRAC merge gate';
const MARKER = '<!-- airac-gate-bot -->';

function validDate(date) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return false;
  const value = new Date(`${date}T00:00:00Z`);
  return !Number.isNaN(value.getTime()) && value.toISOString().slice(0, 10) === date;
}

// Accept only the documented date: "cycle" format, not arbitrary YAML.
function parseCycles(text) {
  const cycles = new Map();
  const dates = new Set();
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim() || line.trimStart().startsWith('#')) continue;
    const match = line.match(/^(\d{4}-\d{2}-\d{2}):\s*"(\d{4})"\s*$/);
    if (!match || !validDate(match[1])) throw new Error(`Invalid AIRAC entry: ${line}`);
    const [, date, cycle] = match;
    if (cycles.has(cycle) || dates.has(date)) throw new Error(`Duplicate AIRAC entry: ${line}`);
    cycles.set(cycle, date);
    dates.add(date);
  }
  if (!cycles.size) throw new Error('The AIRAC calendar is empty');
  return cycles;
}

function readCycles() {
  return parseCycles(fs.readFileSync('.github/airac-cycles.yml', 'utf8'));
}

function labelNames(pull) {
  return pull.labels.map(label => typeof label === 'string' ? label : label.name);
}

function gateResult(pull, cycles, now = new Date()) {
  const labels = labelNames(pull);
  const dates = [];
  for (const label of labels.filter(name => name.startsWith('airac/'))) {
    const cycle = label.slice('airac/'.length);
    if (!cycles.has(cycle)) {
      return { state: 'failure', description: 'Unknown AIRAC label; use a cycle from .github/airac-cycles.yml' };
    }
    dates.push(cycles.get(cycle));
  }

  // Keep existing labelled PRs blocked during migration to cycle labels.
  if (labels.includes('airac-dependent')) {
    const match = (pull.body ?? '').match(/AIRAC effective date[^\n]*:\s*`?(\d{4}-\d{2}-\d{2})`?/i);
    if (!match || !validDate(match[1])) {
      return { state: 'failure', description: 'Replace airac-dependent with an airac/XXXX cycle label' };
    }
    dates.push(match[1]);
  }

  if (!dates.length) return { state: 'success', description: 'This pull request is not AIRAC-dependent' };
  const date = dates.sort().at(-1);
  const cycle = [...cycles].find(([, effective]) => effective === date)?.[0];
  return now.getTime() < new Date(`${date}T00:00:00Z`).getTime()
    ? { state: 'pending', date, cycle, description: `Blocked until ${date} at 00:00 UTC` }
    : { state: 'success', date, cycle, description: `AIRAC effective date ${date} has been reached` };
}

async function updateComment({ github, context, pull, result }) {
  const { owner, repo } = context.repo;
  const comments = await github.paginate(github.rest.issues.listComments, {
    owner, repo, issue_number: pull.number, per_page: 100,
  });
  const previous = comments.find(comment =>
    comment.user?.login === 'github-actions[bot]' && comment.body?.startsWith(MARKER));
  if (!result.date && result.state === 'success' && !previous) return;

  let message;
  const cycleName = result.cycle ? `AIRAC cycle ${result.cycle}` : 'AIRAC effective date';
  if (result.state === 'pending') {
    message = `> [!CAUTION]\n> **Merge blocked** - This PR targets **${cycleName}** (effective ${result.date} at 00:00 UTC).\n>\n> The merge check will pass automatically once the cycle date arrives, on the next scheduled evaluation (every 15 minutes, subject to GitHub scheduling delays).\n> You can [enable auto-merge](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request) now - the PR will merge on its own once all required reviews and checks pass.`;
    if (result.cycle) {
      const query = encodeURIComponent(`is:pr is:open label:airac/${result.cycle}`);
      message += `\n>\n> [View all PRs for AIRAC ${result.cycle}](https://github.com/${owner}/${repo}/pulls?q=${query})`;
    }
  } else if (result.state === 'failure') {
    message = `> [!WARNING]\n> ${result.description}`;
  } else {
    message = `> [!TIP]\n> ${result.date ? `**${cycleName} is now effective (${result.date}).**` : 'No AIRAC dependency label remains on this PR.'}\n> The merge gate has been lifted. If auto-merge is enabled, this PR will merge once all required reviews and checks pass.`;
  }
  const body = `${MARKER}\n${message}`;
  if (previous?.body === body) return;
  if (previous) {
    await github.rest.issues.updateComment({ owner, repo, comment_id: previous.id, body });
  } else {
    await github.rest.issues.createComment({ owner, repo, issue_number: pull.number, body });
  }
}

async function evaluate({ github, context, core, cycles, now = new Date() }) {
  let calendarError;
  try { cycles ??= readCycles(); } catch (error) { calendarError = error; }
  const { owner, repo } = context.repo;
  const pulls = context.payload.pull_request
    ? [context.payload.pull_request]
    : await github.paginate(github.rest.pulls.list, { owner, repo, state: 'open', base: 'main', per_page: 100 });
  for (const candidate of pulls) {
    try {
      // Webhook/scheduled payloads may be stale after label edits or autofix pushes.
      const { data: pull } = await github.rest.pulls.get({ owner, repo, pull_number: candidate.number });
      if (pull.state !== 'open' || pull.base.ref !== 'main') continue;
      const result = calendarError
        ? { state: 'failure', description: 'Invalid AIRAC calendar; ask Navigation to correct it' }
        : gateResult(pull, cycles, now);
      // Repeated schedules must not exhaust GitHub's per-SHA/context status limit.
      const statuses = await github.paginate(github.rest.repos.listCommitStatusesForRef, {
        owner, repo, ref: pull.head.sha, per_page: 100,
      });
      const previous = statuses.find(status => status.context === STATUS);
      if (previous?.state !== result.state || previous?.description !== result.description) {
        await github.rest.repos.createCommitStatus({
          owner, repo, sha: pull.head.sha, context: STATUS,
          state: result.state, description: result.description,
        });
      }
      if (labelNames(pull).some(name => name.startsWith('airac/') || name === 'airac-dependent')
          || context.payload.action === 'unlabeled') {
        await updateComment({ github, context, pull, result });
      }
      core.info(`#${pull.number}: ${result.state} - ${result.description}`);
    } catch (error) {
      core.setFailed(`#${candidate.number}: ${error.message}`);
    }
  }
  if (calendarError) core.setFailed(calendarError.message);
}

async function syncLabels({ github, context, core, maxUpcoming = 4, cycles = readCycles(), now = new Date() }) {
  if (!Number.isInteger(maxUpcoming) || maxUpcoming < 1 || maxUpcoming > 52) {
    throw new Error('max_upcoming must be an integer between 1 and 52');
  }
  const { owner, repo } = context.repo;
  const today = now.toISOString().slice(0, 10);
  const upcoming = [...cycles].filter(([, date]) => date >= today)
    .sort((a, b) => a[1].localeCompare(b[1])).slice(0, maxUpcoming);
  if (!upcoming.length) throw new Error('No upcoming AIRAC cycles; extend .github/airac-cycles.yml');
  const wanted = new Map(upcoming.map(([cycle, date]) => [`airac/${cycle}`, date]));
  const labels = await github.paginate(github.rest.issues.listLabelsForRepo, { owner, repo, per_page: 100 });
  const existing = new Map(labels.map(label => [label.name, label]));
  for (const [name, date] of wanted) {
    const description = `AIRAC ${name.slice(6)} - effective ${date}`;
    const color = 'DC521D';
    if (!existing.has(name)) {
      await github.rest.issues.createLabel({ owner, repo, name, description, color });
    } else if (existing.get(name).description !== description || existing.get(name).color.toUpperCase() !== color) {
      await github.rest.issues.updateLabel({ owner, repo, name, description, color });
    }
  }
  // Never remove a label still protecting an open PR, even after its cycle date.
  const pulls = await github.paginate(github.rest.pulls.list, { owner, repo, state: 'open', per_page: 100 });
  const inUse = new Set(pulls.flatMap(labelNames));
  for (const name of existing.keys()) {
    if (/^airac\/\d{4}$/.test(name) && cycles.has(name.slice(6)) && !wanted.has(name) && !inUse.has(name)) {
      await github.rest.issues.deleteLabel({ owner, repo, name });
    }
  }
  core.info(`Synced ${wanted.size} upcoming AIRAC labels; labels on open PRs were preserved.`);
}

async function tagCycle({ github, context, core, cycles = readCycles(), now = new Date() }) {
  const today = now.toISOString().slice(0, 10);
  const cycle = [...cycles].find(([, date]) => date === today)?.[0];
  if (!cycle) { core.info(`No AIRAC cycle starts on ${today}.`); return; }
  const { owner, repo } = context.repo;
  const tag = `airac-${cycle}`;
  try {
    await github.rest.git.getRef({ owner, repo, ref: `tags/${tag}` });
    core.info(`${tag} already exists; leaving it unchanged.`);
    return;
  } catch (error) { if (error.status !== 404) throw error; }
  const { data: main } = await github.rest.git.getRef({ owner, repo, ref: 'heads/main' });
  const { data: object } = await github.rest.git.createTag({
    owner, repo, tag, message: `AIRAC cycle ${cycle} - effective ${today}`,
    object: main.object.sha, type: 'commit',
  });
  await github.rest.git.createRef({ owner, repo, ref: `refs/tags/${tag}`, sha: object.sha });
  core.info(`Created ${tag} at ${main.object.sha}. This is a snapshot, not an installer release.`);
}

module.exports = { parseCycles, gateResult, evaluate, syncLabels, tagCycle };
