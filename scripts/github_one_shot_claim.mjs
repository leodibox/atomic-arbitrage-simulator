// GitHub-ref-based one-shot claim for a future, explicitly armed automatic
// arb executor. No extra Render service, database, signing, or fund movement.
// IMPORTANT: This alone is NOT a production approval. The fixed ref must be
// protected against deletion by a GitHub repository ruleset, and a separate
// exact-transaction safety gate must pass before any broadcast.
const REPO = "leodibox/atomic-arbitrage-simulator";
export const CLAIM_REF = "refs/heads/first-live-gas-budget-consumed";
const API = `https://api.github.com/repos/${REPO}/git/refs`;

function assertSha(value) {
  if (!/^[\da-f]{40}$/i.test(value ?? "")) {
    throw new Error("A pinned reviewed commit SHA is required");
  }
}
function failed(message) {
  throw new Error(`ONE_SHOT_CLAIM_DENIED: ${message}`);
}
async function request(url, token, method, body, fetchImpl) {
  let response;
  try {
    response = await fetchImpl(url, {
      method,
      headers: {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${token}`,
        "X-GitHub-Api-Version": "2022-11-28",
        ...(body ? {"Content-Type": "application/json"} : {}),
      },
      ...(body ? {body: JSON.stringify(body)} : {}),
      signal: AbortSignal.timeout(6000),
    });
  } catch {
    failed("GitHub unreachable or timeout; never retry/submit");
  }
  if (!response.ok) {
    // Intentionally suppress response bodies, which can contain sensitive info.
    failed(`GitHub responded HTTP ${response.status}; never retry/submit`);
  }
  try {
    return await response.json();
  } catch {
    failed("GitHub response could not be verified; never submit");
  }
}

// Explicitly one POST, one GET. Ambiguous POST outcome FAILS CLOSED.
// Caller must first verify owner, signer, exact candidate, gas and profits,
// and only call once at the moment of arming the first authorized trial.
// The returned 'claimed' state does NOT authorize a transaction by itself.
export async function claimInitialTrial({
  token,
  reviewedCommitSha,
  repositoryRulesetVerified = false,
  executionSafetyChecksPassed = false,
  fetchImpl = fetch,
}) {
  if (typeof token !== "string" || token.length < 16) {
    failed("a dedicated restricted GitHub credential is required");
  }
  assertSha(reviewedCommitSha);
  if (!repositoryRulesetVerified) {
    failed("deletion of the permanent claim ref must be blocked by reviewed ruleset");
  }
  if (!executionSafetyChecksPassed) {
    failed("candidate, contract, exact eth_call, oracle and $5 gas gates must pass first");
  }
  const created = await request(
    API, token, "POST",
    {ref: CLAIM_REF, sha: reviewedCommitSha},
    fetchImpl
  );
  if (created?.ref !== CLAIM_REF || created?.object?.sha?.toLowerCase() !== reviewedCommitSha.toLowerCase()) {
    failed("newly created claim ref was not the exact expected commit");
  }
  const verified = await request(
    `${API}/heads/first-live-gas-budget-consumed`,
    token, "GET", null, fetchImpl
  );
  if (verified?.ref !== CLAIM_REF || verified?.object?.sha?.toLowerCase() !== reviewedCommitSha.toLowerCase()) {
    failed("permanent consumed marker cannot be read back");
  }
  return {claimed: true, ref: CLAIM_REF, reviewedCommitSha};
}
