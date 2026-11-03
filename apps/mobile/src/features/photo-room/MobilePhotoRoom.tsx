import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import { PhotoRoomEditor, type PhotoRoomModelOptions, type PhotoRoomPreparedMedia } from "@volum/photo-room";
import { MobileModelViewer } from "../../components/MobileModelViewer";
import { t } from "../../lib/i18n";
import { mutateJson, uploadProjectPhoto } from "../../lib/transport";
import type { Credential, ProjectDocumentation } from "../../types";

interface MobilePhotoRoomProps {
  title: string;
  credential?: Credential;
  assetId?: string;
  revision?: string;
  plateIndex?: number;
  offlinePayload?: ArrayBuffer;
  localPayload?: ArrayBuffer;
  projectId?: string;
  documentationRevision?: string;
  canSaveToProject?: boolean;
  onClose: () => void;
}

export function MobilePhotoRoom({ title, credential, assetId, revision = "local", plateIndex, offlinePayload, localPayload, projectId, documentationRevision, canSaveToProject = false, onClose }: MobilePhotoRoomProps) {
  const queryClient = useQueryClient();
  const onStatus = useCallback((_status: "ready" | "error", _message?: string) => undefined, []);
  const renderModel = (options: PhotoRoomModelOptions) => <MobileModelViewer
    credential={credential}
    assetId={assetId}
    revision={revision}
    plateIndex={plateIndex}
    offlinePayload={offlinePayload}
    localPayload={localPayload}
    resetSignal={options.resetSignal}
    capture={options.capture}
    hideBuildPlate
    fitToFrame
    modelOrientation={options.orientation}
    interactionEnabled={options.interactionEnabled}
    onStatus={onStatus}
  />;
  const saveFile = async (file: File) => {
    const extension = file.name.split(".").pop() || (file.type === "video/mp4" ? "mp4" : "jpg");
    const path = await save({ defaultPath: file.name, filters: [{ name: file.type === "video/mp4" ? "MP4 video" : "JPEG image", extensions: [extension] }] });
    if (!path) return;
    await writeFile(path, new Uint8Array(await file.arrayBuffer()));
  };
  const deliver = async ({ file }: PhotoRoomPreparedMedia) => {
    const data: ShareData = { title, files: [file] };
    if (navigator.share && (!navigator.canShare || navigator.canShare(data))) {
      await navigator.share(data);
      return;
    }
    await saveFile(file);
  };
  const saveToProject = async (file: File, useAsCover: boolean) => {
    if (!credential || !projectId || !documentationRevision) throw new Error("Connect to Volum Desktop before adding this photo to the project.");
    const uploadId = crypto.randomUUID();
    let updated = await uploadProjectPhoto(credential, file, projectId, "Created in Photo Room", documentationRevision, () => undefined, uploadId);
    const added = updated.photos.find((photo) => photo.originalFilename === file.name) ?? updated.photos.at(-1);
    if (useAsCover && added && !added.isCover) {
      updated = await mutateJson<ProjectDocumentation>(credential, `/api/v1/projects/${encodeURIComponent(projectId)}/photos/${encodeURIComponent(added.id)}`, "PATCH", {
        caption: added.caption,
        sortOrder: added.sortOrder,
        isCover: true,
        ifUnmodifiedSince: updated.revision,
      });
    }
    queryClient.setQueryData(["documentation", projectId], updated);
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["project", projectId] }),
      queryClient.invalidateQueries({ queryKey: ["projects"] }),
    ]);
  };
  return <PhotoRoomEditor
    title={title}
    mobile
    canSaveToProject={canSaveToProject && Boolean(credential && projectId && documentationRevision)}
    renderModel={renderModel}
    onClose={onClose}
    onDeliver={deliver}
    onSaveToProject={saveToProject}
    translate={t}
  />;
}
