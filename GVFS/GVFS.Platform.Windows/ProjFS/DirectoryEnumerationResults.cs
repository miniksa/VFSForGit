using System;
using System.IO;
using System.Runtime.InteropServices;
using static Microsoft.Windows.ProjFS.ProjFSNative;

namespace Microsoft.Windows.ProjFS
{
    /// <summary>
    /// Pure C# P/Invoke implementation of IDirectoryEnumerationResults,
    /// wrapping PrjFillDirEntryBuffer for the given PRJ_DIR_ENTRY_BUFFER_HANDLE.
    /// </summary>
    public class DirectoryEnumerationResults : IDirectoryEnumerationResults
    {
        private readonly IntPtr _dirEntryBufferHandle;

        internal DirectoryEnumerationResults(IntPtr dirEntryBufferHandle)
        {
            _dirEntryBufferHandle = dirEntryBufferHandle;
        }

        public bool Add(
            string fileName,
            long fileSize,
            bool isDirectory,
            FileAttributes fileAttributes,
            DateTime creationTime,
            DateTime lastAccessTime,
            DateTime lastWriteTime,
            DateTime changeTime)
        {
            var basicInfo = new PRJ_FILE_BASIC_INFO
            {
                IsDirectory = isDirectory ? (byte)1 : (byte)0,
                FileSize = fileSize,
                CreationTime = creationTime.ToFileTimeUtc(),
                LastAccessTime = lastAccessTime.ToFileTimeUtc(),
                LastWriteTime = lastWriteTime.ToFileTimeUtc(),
                ChangeTime = changeTime.ToFileTimeUtc(),
                FileAttributes = (uint)fileAttributes,
            };

            int hr = ProjFSNative.PrjFillDirEntryBuffer(fileName, ref basicInfo, _dirEntryBufferHandle);
            return hr >= 0; // S_OK = success; negative HRESULT (e.g. INSUFFICIENT_BUFFER) = buffer full
        }

        public bool Add(string fileName, long fileSize, bool isDirectory)
        {
            return Add(
                fileName,
                fileSize,
                isDirectory,
                isDirectory ? FileAttributes.Directory : FileAttributes.Archive,
                DateTime.UtcNow,
                DateTime.UtcNow,
                DateTime.UtcNow,
                DateTime.UtcNow);
        }
    }
}
