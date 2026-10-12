import test from "node:test";
import assert from "node:assert/strict";
import { amountText, decimalDifference, integerDifference } from "./economic-amounts.js";

test("decimal differences preserve arbitrary precision and fractional scale", () => {
  assert.equal(decimalDifference("900719925474099300.12", "900719925474099299.92"), "0.2");
  assert.equal(decimalDifference("1.200", "2.05"), "-0.85");
  assert.equal(decimalDifference("-0.25", "-0.4"), "0.15");
  assert.equal(decimalDifference({ value: "100000000000000000000" }, "0.01"), "99999999999999999999.99");
  assert.equal(decimalDifference("NaN", "0"), null);
  assert.equal(amountText(null), "—");
});

test("integer deltas do not round unsafe JavaScript numbers", () => {
  assert.equal(integerDifference("900719925474099300", "900719925474099299"), "1");
  assert.equal(integerDifference(42, 40), "2");
  assert.equal(integerDifference(Number.MAX_SAFE_INTEGER + 1, 0), null);
});
