import { useEffect, useState } from "react";
import { Link, Outlet } from "react-router";
import { getEnvironment } from "../data";
import { InternalLink } from "./ui";

export function Layout() {
  const [environment, setEnvironment] = useState("");
  useEffect(() => {
    getEnvironment().then(setEnvironment, () => setEnvironment("unknown"));
  }, []);

  return (
    <div className="min-h-screen bg-white px-6 py-5 text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
      <header className="mb-6 flex items-baseline gap-6 border-b border-neutral-200 pb-3 dark:border-neutral-700">
        <h1 className="text-2xl font-bold">
          <Link to="/">
            epubize <small className="text-sm font-normal text-neutral-500">{environment}</small>
          </Link>
        </h1>
        <nav className="flex gap-4">
          <InternalLink to="/episodes/latest">latest</InternalLink>
          <InternalLink to="/settings">settings</InternalLink>
        </nav>
      </header>
      <main>
        <Outlet />
      </main>
    </div>
  );
}
