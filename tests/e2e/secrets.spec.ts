import { test, expect } from "@playwright/test";
import { defaultMockState, installTauriMock, populatedState } from "./setup";

test.describe("Secrets", () => {
  test("add Local secret", async ({ page }) => {
    await installTauriMock(page, defaultMockState());
    await page.goto("/secrets");

    await expect(page.getByText("No secrets stored yet")).toBeVisible();

    await page.getByRole("button", { name: /add secret/i }).click();

    await page.getByPlaceholder("e.g., github-pat").fill("github-pat");
    await page
      .getByPlaceholder("e.g., GitHub Personal Access Token")
      .fill("GitHub PAT");
    await page.getByPlaceholder("Paste your API key or token").fill("ghp_xxx");

    await page.getByRole("button", { name: "Save Secret" }).click();

    await expect(page.getByText("GitHub PAT", { exact: true })).toBeVisible();
    await expect(page.getByText("github-pat").first()).toBeVisible();
  });

  test("add 1Password reference secret (no value field)", async ({ page }) => {
    await installTauriMock(page, defaultMockState());
    await page.goto("/secrets");

    await page.getByRole("button", { name: /add secret/i }).click();

    // Switch to 1Password source.
    await page.getByRole("button", { name: /^1Password$/ }).click();

    await page.getByPlaceholder("e.g., github-pat").fill("openai");
    await page
      .getByPlaceholder("e.g., GitHub Personal Access Token")
      .fill("OpenAI key");
    await page
      .getByPlaceholder("op://vault/item/field")
      .fill("op://Personal/OpenAI/key");

    // Ensure the Local-only password input is absent.
    await expect(
      page.getByPlaceholder("Paste your API key or token"),
    ).toBeHidden();

    await page.getByRole("button", { name: "Save Reference" }).click();

    await expect(page.getByText("OpenAI key", { exact: true })).toBeVisible();
    await expect(page.getByText(/op:\/\/Personal\/OpenAI\/key/)).toBeVisible();
  });

  test("delete secret", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/secrets");

    await expect(page.getByText("GitHub PAT", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Delete GitHub PAT" }).click();

    await expect(page.getByText("GitHub PAT", { exact: true })).toBeHidden();
  });

  test("search narrows the list", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/secrets");

    await expect(page.getByText("GitHub PAT", { exact: true })).toBeVisible();
    await expect(page.getByText("OpenAI API Key", { exact: true })).toBeVisible();

    await page
      .getByPlaceholder("Search by id, label, source…")
      .fill("github");

    await expect(page.getByText("GitHub PAT", { exact: true })).toBeVisible();
    await expect(
      page.getByText("OpenAI API Key", { exact: true }),
    ).toBeHidden();
  });
});
