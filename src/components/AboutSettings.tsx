import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, RefreshCw, Shuffle } from "lucide-react";
import packageMetadata from "../../package.json";
import volumIcon from "../assets/volum-icon-transparent.png";
import { api } from "../lib/tauri/api";
import { fetchRandomDidacProjects, type DidacProject } from "../lib/didac-projects";
import { t } from "../lib/i18n";

function ProjectImage({ project }: { project: DidacProject }) {
  const [failed, setFailed] = useState(false);
  if (failed) return <span className="about-project__fallback">{project.name.slice(0, 1).toUpperCase()}</span>;
  return <img src={project.imageUrl} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setFailed(true)} />;
}

export function AboutSettings() {
  const { data: projects = [], isLoading, isFetching, error, refetch } = useQuery({
    queryKey: ["didac-projects"],
    queryFn: ({ signal }) => fetchRandomDidacProjects(signal),
    staleTime: Number.POSITIVE_INFINITY,
    retry: 1
  });
  return <section className="settings-section about-section" id="about"><div className="about-hero"><img src={volumIcon} alt="Volum" /><div><div className="eyebrow">Volum {packageMetadata.version}</div><h2>{t("A beautiful home for your 3D models.")}</h2><p>{t("Private, local-first, and built for makers.")}</p></div><button className="button button--quiet" onClick={() => api.openExternal("https://didac.dev/")}>{t("Built by didac.dev")} <ExternalLink size={14} /></button></div><div className="about-projects__header"><div><h3>{t("More from didac.dev")}</h3><p>{t("Five randomly selected projects from didac.dev.")}</p></div><button className="button button--quiet" disabled={isFetching} onClick={() => void refetch()}><Shuffle size={14} /> {t("Shuffle")}</button></div>{isLoading ? <div className="about-projects about-projects--loading">{Array.from({ length: 5 }, (_, index) => <div className="about-project skeleton" key={index} />)}</div> : error ? <div className="about-projects__error"><span>{t("Couldn’t load projects right now.")}</span><button className="button button--quiet" onClick={() => void refetch()}><RefreshCw size={14} /> {t("Try again")}</button></div> : <div className="about-projects">{projects.map((project) => <button className="about-project" key={`${project.name}-${project.url}`} onClick={() => api.openExternal(project.url)}><span className="about-project__image"><ProjectImage project={project} /></span><span><strong>{project.name}</strong><small>{project.description}</small></span><ExternalLink size={14} /></button>)}</div>}<p className="about-network-note">{t("This list is fetched from")} <strong>didac.dev/api/projects</strong>. {t("No library information is included in the request.")}</p></section>;
}
