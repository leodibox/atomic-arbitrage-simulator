import test from "node:test";
import assert from "node:assert/strict";
import {claimInitialTrial, CLAIM_REF} from "./github_one_shot_claim.mjs";

const sha = "a".repeat(40);
function response(status, body) {
  return {ok: status >= 200 && status < 300, status,
    async json() {return body;}};
}
const good = {ref:CLAIM_REF, object:{sha}};
const args = {token:"example-restricted-github-test-token",
  reviewedCommitSha:sha, repositoryRulesetVerified:true,
  executionSafetyChecksPassed:true};

test("creates permanent claim once, reads back exact Git ref", async () => {
  let count=0;
  const fetchImpl = async (url, request) => {
    if (++count === 1) {
      assert.equal(request.method,"POST");
      assert.deepEqual(JSON.parse(request.body),{ref:CLAIM_REF,sha});
      return response(201,good);
    }
    assert.equal(request.method,"GET");
    assert.match(url,/\/heads\/first-live-gas-budget-consumed$/);
    return response(200,good);
  };
  const result=await claimInitialTrial({...args,fetchImpl});
  assert.equal(result.claimed,true);
  assert.equal(count,2);
});

test("denies second claim when branch already exists, without GET or retry",async()=>{
  let count=0;
  await assert.rejects(claimInitialTrial({...args,fetchImpl:async()=>{
    count++; return response(422,{message:"Reference already exists"});
  }}),/ONE_SHOT_CLAIM_DENIED/);
  assert.equal(count,1);
});

test("ambiguous timeout never retries",async()=>{
  let count=0;
  await assert.rejects(claimInitialTrial({...args,fetchImpl:async()=>{
    count++;throw Error("network timeout after ref created");
  }}),/ONE_SHOT_CLAIM_DENIED/);
  assert.equal(count,1);
});

test("rejects wrong created ref or incorrect readback",async()=>{
  await assert.rejects(claimInitialTrial({...args,fetchImpl:async()=>
    response(201,{ref:CLAIM_REF,object:{sha:"b".repeat(40)}})}),/newly created claim/);
  let count=0;
  await assert.rejects(claimInitialTrial({...args,fetchImpl:async()=>{
    count++;
    return count===1?response(201,good):response(200,{ref:CLAIM_REF,object:{sha:"b".repeat(40)}});
  }}),/cannot be read back/);
});

test("rejects preflight failures without making a request",async()=>{
  const fetchImpl=()=>{throw Error("must not be invoked")};
  await assert.rejects(claimInitialTrial({...args,repositoryRulesetVerified:false,fetchImpl}),/deletion/);
  await assert.rejects(claimInitialTrial({...args,executionSafetyChecksPassed:false,fetchImpl}),/candidate/);
  await assert.rejects(claimInitialTrial({...args,reviewedCommitSha:"wrong",fetchImpl}),/pinned/);
  await assert.rejects(claimInitialTrial({...args,token:"",fetchImpl}),/credential/);
});
