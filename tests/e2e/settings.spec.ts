import { test, expect } from "@playwright/test";
import { defaultMockState, installTauriMock, populatedState } from "./setup";

test.describe("Settings", () => {
  test("renders recent audit log entries", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/settings");

    await expect(
      page.getByRole("heading", { name: "Audit Log" }),
    ).toBeVisible();
    await expect(page.getByText("github", { exact: true })).toBeVisible();
    await expect(page.getByText("github-pat", { exact: true })).toBeVisible();
    await expect(page.getByText("Success", { exact: true })).toBeVisible();
  });

  test("shows empty audit log state", async ({ page }) => {
    await installTauriMock(page, defaultMockState());
    await page.goto("/settings");

    await expect(
      page.getByText(/No audit log entries yet/i),
    ).toBeVisible();
  });
});
