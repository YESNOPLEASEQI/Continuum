import { Navigate, Route, Routes } from "react-router-dom";
import AppShell from "./layout/AppShell";
import ProjectsPage from "./pages/ProjectsPage";
import UnifiedChatPage from "./pages/UnifiedChatPage";
import NewContinuationPage from "./pages/NewContinuationPage";
import ContextInspectorPage from "./pages/ContextInspectorPage";
import ConfigurationsPage from "./pages/ConfigurationsPage";
import SessionDetailPage from "./pages/SessionDetailPage";
import SessionsPage from "./pages/SessionsPage";
import SettingsPage from "./pages/SettingsPage";
import ProfilesPage from "./pages/ProfilesPage";
import SearchPage from "./pages/SearchPage";
import DiagnosticsPage from "./pages/DiagnosticsPage";

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<Navigate to="/projects" replace />} />
        <Route path="projects" element={<ProjectsPage />} />
        <Route path="projects/:id/chat" element={<UnifiedChatPage />} />
        <Route path="projects/:id/continuation" element={<NewContinuationPage />} />
        <Route path="projects/:id/context" element={<ContextInspectorPage />} />
        <Route path="sessions" element={<SessionsPage />} />
        <Route path="sessions/:id" element={<SessionDetailPage />} />
        <Route path="configurations" element={<ConfigurationsPage />} />
        <Route path="profiles" element={<ProfilesPage />} />
        <Route path="search" element={<SearchPage />} />
        <Route path="diagnostics" element={<DiagnosticsPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/projects" replace />} />
      </Route>
    </Routes>
  );
}
