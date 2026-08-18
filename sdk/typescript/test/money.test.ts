import { test } from "node:test";
import assert from "node:assert/strict";
import {
  add, changeBps, compare, formatPrice, fromJsonNumber, MoneyError,
  parsePrice, price, rescale, subtract, toNumber,
} from "../src/money.ts";

test("parsePrice is exact", () => {
  assert.equal(parsePrice("35.42", "GHS", 2).value, 3542n);
  assert.equal(parsePrice("0.14", "GHS", 4).value, 1400n);
  assert.equal(parsePrice("-1.5", "NGN", 4).value, -15000n);
  assert.equal(parsePrice("78", "GHS", 2).value, 7800n);
  assert.equal(parsePrice("1,234.56", "NGN", 2).value, 123456n);
  assert.equal(parsePrice(".5", "NGN", 2).value, 50n);
});

test("parsePrice refuses to silently drop precision", () => {
  assert.throws(() => parsePrice("1.23456", "GHS", 2), MoneyError);
  assert.throws(() => parsePrice("abc", "GHS"), MoneyError);
  assert.throws(() => parsePrice("", "GHS"), MoneyError);
});

test("the float problem this module exists to avoid", () => {
  // 0.1 + 0.2 !== 0.3 in binary floating point.
  assert.notEqual(0.1 + 0.2, 0.3);
  const sum = add(parsePrice("0.1", "GHS", 4), parsePrice("0.2", "GHS", 4));
  assert.ok(compare(sum, parsePrice("0.3", "GHS", 4)) === 0);
});

test("fromJsonNumber handles what the kwayisi API actually returns", () => {
  assert.equal(fromJsonNumber(0.42, "GHS", 4).value, 4200n);
  assert.equal(fromJsonNumber(37.0, "GHS", 4).value, 370000n);
  assert.equal(fromJsonNumber(-0.01, "GHS", 4).value, -100n);
  assert.equal(fromJsonNumber(93050310601.5, "GHS", 4).value, 930503106015000n);
});

test("rescale rounds half away from zero", () => {
  assert.equal(rescale(price(12345n, 4, "NGN"), 2).value, 123n);   // 1.2345 -> 1.23
  assert.equal(rescale(price(12350n, 4, "NGN"), 2).value, 124n);   // 1.2350 -> 1.24
  assert.equal(rescale(price(-12350n, 4, "NGN"), 2).value, -124n); // away from zero
  assert.equal(rescale(price(123n, 2, "NGN"), 4).value, 12300n);
});

test("mixed scales align before arithmetic", () => {
  const a = price(150n, 2, "NGN");    // 1.50
  const b = price(12345n, 4, "NGN");  // 1.2345
  assert.equal(subtract(a, b).value, 2655n); // 0.2655 at scale 4
  assert.equal(subtract(a, b).scale, 4);
});

test("currency mismatch is an error, not a coincidence", () => {
  assert.throws(() => add(price(1n, 2, "NGN"), price(1n, 2, "GHS")), MoneyError);
});

test("changeBps", () => {
  // 0.77 from a previous close of 0.78 is -128 bps.
  const prev = parsePrice("0.78", "GHS", 4);
  const last = parsePrice("0.77", "GHS", 4);
  assert.equal(changeBps(prev, last), -128n);
  assert.throws(() => changeBps(price(0n, 2, "GHS"), last), MoneyError);
});

test("formatting and display conversion", () => {
  assert.equal(formatPrice(parsePrice("1234.5", "NGN", 2)), "NGN 1,234.50");
  assert.equal(formatPrice(parsePrice("-7.03", "GHS", 2), { symbol: false }), "-7.03");
  assert.equal(toNumber(parsePrice("7.03", "GHS", 4)), 7.03);
});
