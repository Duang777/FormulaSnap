// Property-based tests for useImageInput
// Task 13.2: Invalid image input rejection (fast-check)
// Validates: Requirements 2.3

import { describe, it, expect } from "vitest";
import fc from "fast-check";
import {
  isValidImageType,
  VALID_IMAGE_TYPES,
} from "./useImageInput";

describe("isValidImageType - Property Tests", () => {
  it("Property 1: all VALID_IMAGE_TYPES are accepted", () => {
    fc.assert(
      fc.property(
        fc.constantFrom(...VALID_IMAGE_TYPES),
        (mimeType) => {
          expect(isValidImageType(mimeType)).toBe(true);
        }
      )
    );
  });

  it("Property 1: random non-image MIME types are rejected", () => {
    const nonImageMimeArb = fc
      .string({ minLength: 1 })
      .filter(
        (s) =>
          !VALID_IMAGE_TYPES.includes(s as (typeof VALID_IMAGE_TYPES)[number])
      );

    fc.assert(
      fc.property(nonImageMimeArb, (mimeType) => {
        expect(isValidImageType(mimeType)).toBe(false);
      })
    );
  });

  it("Property 1: common non-image types are all rejected", () => {
    const nonImageTypes = fc.constantFrom(
      "text/plain",
      "text/html",
      "text/csv",
      "application/pdf",
      "application/json",
      "application/xml",
      "application/javascript",
      "application/zip",
      "audio/mpeg",
      "video/mp4",
      "image/svg+xml",
      "image/tiff",
      "",
    );

    fc.assert(
      fc.property(nonImageTypes, (mimeType) => {
        expect(isValidImageType(mimeType)).toBe(false);
      })
    );
  });
});
