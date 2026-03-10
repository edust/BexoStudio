import { AppProviders } from "@/app/providers";
import { AppRouter } from "@/routes/app-router";

export function App() {
  return (
    <AppProviders>
      <AppRouter />
    </AppProviders>
  );
}
