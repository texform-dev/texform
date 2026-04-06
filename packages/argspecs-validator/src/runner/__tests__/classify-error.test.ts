import { describe, test, expect } from "bun:test";
import { classifyError } from "../result-collector";

describe("classifyError", () => {
  // unsupported
  test("Undefined control sequence → unsupported", () => {
    expect(classifyError("Undefined control sequence: \\foo", "baseline", true)).toBe("unsupported");
  });

  test("No such environment → unsupported", () => {
    expect(classifyError("No such environment: align", "baseline", true)).toBe("unsupported");
  });

  test("Environment xxx undefined → unsupported", () => {
    expect(classifyError("Environment align undefined", "baseline", true)).toBe("unsupported");
  });

  test("Unknown command → unsupported", () => {
    expect(classifyError("Unknown command: \\foo", "baseline", true)).toBe("unsupported");
  });

  // syntax_divergence
  test("Illegal unit of measure → syntax_divergence", () => {
    expect(classifyError("Illegal unit of measure (pt inserted)", "bare[0]", true)).toBe("syntax_divergence");
  });

  test("A <box> was supposed to be here → syntax_divergence", () => {
    expect(classifyError("A <box> was supposed to be here", "bare[1]", true)).toBe("syntax_divergence");
  });

  test("bare branch failure with baseline passing → syntax_divergence", () => {
    expect(classifyError("some other error", "bare[0]", true)).toBe("syntax_divergence");
  });

  test("vary branch failure with baseline passing → syntax_divergence", () => {
    expect(classifyError("some other error", "vary:star", true)).toBe("syntax_divergence");
  });

  test("Invalid size on bare branch → syntax_divergence", () => {
    expect(classifyError("Invalid size: 999", "bare[0]", false)).toBe("syntax_divergence");
  });

  // semantic_error
  test("mode error → semantic_error", () => {
    expect(classifyError("allowed only in math mode", "baseline", true)).toBe("semantic_error");
  });

  test("unknown error on baseline → semantic_error", () => {
    expect(classifyError("something went wrong", "baseline", true)).toBe("semantic_error");
  });

  test("bare branch failure when baseline also fails → semantic_error", () => {
    expect(classifyError("something went wrong", "bare[0]", false)).toBe("semantic_error");
  });
});
