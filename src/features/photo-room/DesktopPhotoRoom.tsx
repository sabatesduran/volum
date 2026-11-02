import { save } from "@tauri-apps/plugin-dialog";
import { remove, writeFile } from "@tauri-apps/plugin-fs";
import { appCacheDir, join } from "@tauri-apps/api/path";
import { useQueryClient } from "@tanstack/react-query";
import { PhotoRoomEditor, type PhotoRoomModelOptions, type PhotoRoomPreparedMedia } from "../../../shared/photo-room/PhotoRoomEditor";
import { ModelViewer } from "../../components/ModelViewer";
import { t } from "../../lib/i18n";
import { api, isTauri } from "../../lib/tauri/api";

interface DesktopPhotoRoomProps {
  modelId: string;
  modelName: string;
  assetId?: string;
  extension: string;
  revision?: string;
  plateIndex?: number;
  onClose: () => void;
}

function browserDownload(file: File) {
  const url = URL.createObjectURL(file);
  const link = document.createElement("a");
  link.href = url;
  link.download = file.name;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1_000);
}

export function DesktopPhotoRoom({ modelId, modelName, assetId, extension, revision, plateIndex, onClose }: DesktopPhotoRoomProps) {
  const queryClient = useQueryClient();
  const renderModel = (options: PhotoRoomModelOptions) => <ModelViewer
    assetId={assetId}
    extension={extension}
    modelId={modelId}
    plateIndex={plateIndex}
    revision={revision}
    resetSignal={options.resetSignal}
    capture={options.capture}
    hideBuildPlate
    fitToFrame
    modelOrientation={options.orientation}
    interactionEnabled={options.interactionEnabled}
  />;
  const deliver = async ({ file }: PhotoRoomPreparedMedia) => {
    if (!isTauri()) { browserDownload(file); return; }
    const extension = file.name.split(".").pop() || (file.type === "video/mp4" ? "mp4" : "jpg");
    const path = await save({
      defaultPath: file.name,
      filters: [{ name: file.type === "video/mp4" ? "MP4 video" : "JPEG image", extensions: [extension] }],
    });
    if (!path) return;
    await writeFile(path, new Uint8Array(await file.arrayBuffer()));
  };
  const saveToProject = async (file: File, useAsCover: boolean) => {
    const path = await join(await appCacheDir(), file.name);
    await writeFile(path, new Uint8Array(await file.arrayBuffer()));
    try {
      const photo = await api.addProjectPhoto(modelId, path);
      if (useAsCover && !photo.isCover) {
        await api.updateProjectPhoto({ id: photo.id, caption: photo.caption, sortOrder: photo.sortOrder, isCover: true });
      }
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-documentation", modelId] }),
        queryClient.invalidateQueries({ queryKey: ["model", modelId] }),
        queryClient.invalidateQueries({ queryKey: ["models"] }),
      ]);
    } finally {
      await remove(path).catch(() => undefined);
    }
  };
  return <PhotoRoomEditor title={modelName} renderModel={renderModel} onClose={onClose} onDeliver={deliver} canSaveToProject={isTauri()} onSaveToProject={saveToProject} translate={t} />;
}
