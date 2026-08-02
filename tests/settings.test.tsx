import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import SettingsPage from "../src/pages/SettingsPage";
import { appApi } from "../src/api/bridge";
import { settingsFixture } from "./fixtures";

const { settingsMock, saveSettingsMock, validateSettingsPathsMock } = vi.hoisted(() => ({ settingsMock: vi.fn(), saveSettingsMock: vi.fn(), validateSettingsPathsMock: vi.fn() }));
vi.mock("../src/api/bridge", () => ({ appApi: { settings: settingsMock, saveSettings: saveSettingsMock, validateSettingsPaths: validateSettingsPathsMock } }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

describe("SettingsPage", () => {
  it("persists changed settings through the desktop bridge", async () => {
    settingsMock.mockResolvedValue(settingsFixture);
    saveSettingsMock.mockImplementation(async (value) => value);
    validateSettingsPathsMock.mockResolvedValue([]);
    render(<SettingsPage />);
    expect(await screen.findByText("扫描与监听")).toBeInTheDocument();
    await userEvent.click(screen.getByText("启动时自动扫描"));
    await userEvent.click(screen.getByTestId("save-settings-btn"));
    expect(appApi.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ autoScan: true }));
  });
});
