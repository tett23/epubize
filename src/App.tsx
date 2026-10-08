import { MemoryRouter, Route, Routes } from "react-router";
import { Layout } from "./components/Layout";
import { NormalizeOptionsProvider } from "./normalizeOptions";
import { Latest } from "./pages/Latest";
import { NovelDetail } from "./pages/NovelDetail";
import { NovelEpisode } from "./pages/NovelEpisode";
import { Root } from "./pages/Root";

function App() {
  return (
    <NormalizeOptionsProvider>
      <MemoryRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Root />} />
            <Route path="episodes/latest" element={<Latest />} />
            <Route path="novels/:novelId" element={<NovelDetail />} />
            <Route path="novels/:novelId/:episodeId" element={<NovelEpisode />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </NormalizeOptionsProvider>
  );
}

export default App;
