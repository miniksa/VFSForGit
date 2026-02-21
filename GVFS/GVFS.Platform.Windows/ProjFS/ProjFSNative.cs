// ProjFS Native P/Invoke wrapper for NativeAOT compatibility
// Replaces ProjectedFSLib.Managed.dll (C++/CLI mixed-mode assembly)
// Based on the Windows ProjFS C API: https://learn.microsoft.com/en-us/windows/win32/projfs/projected-file-system

using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace GVFS.Platform.Windows.ProjFS
{
    // ==========================================
    // Enums matching the ProjFS C API
    // ==========================================

    public enum HResult : int
    {
        Ok = 0,
        Pending = unchecked((int)0x00000103),
        InternalError = unchecked((int)0x80004005),
        OutOfMemory = unchecked((int)0x8007000E),
        InvalidArg = unchecked((int)0x80070057),
        FileNotFound = unchecked((int)0x80070002),
        PathNotFound = unchecked((int)0x80070003),
        DirNotEmpty = unchecked((int)0x80070091),
        Handle = unchecked((int)0x80070006),
        VirtualizationInvalidOp = unchecked((int)0x80070371),
    }

    [Flags]
    public enum NotificationType : uint
    {
        None = 0x00000000,
        SuppressNotifications = 0x00000001,
        FileOpened = 0x00000002,
        NewFileCreated = 0x00000004,
        FileOverwritten = 0x00000008,
        PreDelete = 0x00000010,
        PreRename = 0x00000020,
        PreCreateHardlink = 0x00000040,
        FileRenamed = 0x00000080,
        HardlinkCreated = 0x00000100,
        FileHandleClosedNoModification = 0x00000200,
        FileHandleClosedFileModified = 0x00000400,
        FileHandleClosedFileDeleted = 0x00000800,
        FilePreConvertToFull = 0x00001000,
        UseExistingMask = 0xFFFFFFFF,
    }

    [Flags]
    public enum UpdateType : uint
    {
        AllowDirtyMetadata = 0x00000001,
        AllowDirtyData = 0x00000002,
        AllowTombstone = 0x00000004,
        AllowReadOnly = 0x00000008,
    }

    public enum UpdateFailureCause : uint
    {
        NoFailure = 0x00000000,
        DirtyMetadata = 0x00000001,
        DirtyData = 0x00000002,
        Tombstone = 0x00000004,
        ReadOnly = 0x00000008,
    }

    public enum OnDiskFileState : uint
    {
        Full = 0x00000001,
        DirtyPlaceholder = 0x00000002,
        HydratedPlaceholder = 0x00000004,
        Placeholder = 0x00000008,
        Tombstone = 0x00000010,
    }

    public enum PlaceholderIdLength
    {
        SHA1 = 20,
        Length = 128,
    }

    // ==========================================
    // Structs
    // ==========================================

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct PRJ_PLACEHOLDER_INFO
    {
        public PRJ_PLACEHOLDER_VERSION_INFO VersionInfo;
        // EaInformation, SecurityInformation, StreamsInformation are optional and rarely used
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PRJ_PLACEHOLDER_VERSION_INFO
    {
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 128)]
        public byte[] ProviderId;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 128)]
        public byte[] ContentId;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct PRJ_FILE_BASIC_INFO
    {
        public bool IsDirectory;
        public long FileSize;
        public long CreationTime;
        public long LastAccessTime;
        public long LastWriteTime;
        public long ChangeTime;
        public uint FileAttributes;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct PRJ_CALLBACK_DATA
    {
        public uint Size;
        public uint Flags;
        public IntPtr NamespaceVirtualizationContext;
        public int CommandId;
        public Guid FileId;
        public Guid DataStreamId;
        [MarshalAs(UnmanagedType.LPWStr)]
        public string FilePathName;
        public IntPtr VersionInfo;
        public uint TriggeringProcessId;
        [MarshalAs(UnmanagedType.LPWStr)]
        public string TriggeringProcessImageFileName;
        public IntPtr InstanceContext;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PRJ_NOTIFICATION_MAPPING
    {
        public NotificationType NotificationBitMask;
        [MarshalAs(UnmanagedType.LPWStr)]
        public string NotificationRoot;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PRJ_STARTVIRTUALIZING_OPTIONS
    {
        public IntPtr NotificationMappings;
        public uint NotificationMappingsCount;
        public uint PoolThreadCount;
        public uint ConcurrentThreadCount;
        public IntPtr InstanceContext;
    }

    // ==========================================
    // Callback Delegates
    // ==========================================

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_START_DIRECTORY_ENUMERATION_CB(
        in PRJ_CALLBACK_DATA callbackData,
        in Guid enumerationId);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_END_DIRECTORY_ENUMERATION_CB(
        in PRJ_CALLBACK_DATA callbackData,
        in Guid enumerationId);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_GET_DIRECTORY_ENUMERATION_CB(
        in PRJ_CALLBACK_DATA callbackData,
        in Guid enumerationId,
        [MarshalAs(UnmanagedType.LPWStr)] string searchExpression,
        IntPtr dirEntryBufferHandle);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_GET_PLACEHOLDER_INFO_CB(
        in PRJ_CALLBACK_DATA callbackData);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_GET_FILE_DATA_CB(
        in PRJ_CALLBACK_DATA callbackData,
        ulong byteOffset,
        uint length);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_CANCEL_COMMAND_CB(
        in PRJ_CALLBACK_DATA callbackData);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_NOTIFICATION_CB(
        in PRJ_CALLBACK_DATA callbackData,
        bool isDirectory,
        NotificationType notification,
        [MarshalAs(UnmanagedType.LPWStr)] string destinationFileName,
        ref PRJ_NOTIFICATION_PARAMETERS operationParameters);

    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    public delegate HResult PRJ_QUERY_FILE_NAME_CB(
        in PRJ_CALLBACK_DATA callbackData);

    [StructLayout(LayoutKind.Explicit)]
    public struct PRJ_NOTIFICATION_PARAMETERS
    {
        [FieldOffset(0)]
        public PRJ_NOTIFY_TYPES PostCreateNewFile;
        [FieldOffset(0)]
        public PRJ_NOTIFY_TYPES FileRenamed;
        [FieldOffset(0)]
        public PRJ_NOTIFY_TYPES FileDeletedOnHandleClose;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PRJ_NOTIFY_TYPES
    {
        public NotificationType NotificationMask;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PRJ_CALLBACKS
    {
        public PRJ_START_DIRECTORY_ENUMERATION_CB StartDirectoryEnumerationCallback;
        public PRJ_END_DIRECTORY_ENUMERATION_CB EndDirectoryEnumerationCallback;
        public PRJ_GET_DIRECTORY_ENUMERATION_CB GetDirectoryEnumerationCallback;
        public PRJ_GET_PLACEHOLDER_INFO_CB GetPlaceholderInfoCallback;
        public PRJ_GET_FILE_DATA_CB GetFileDataCallback;
        public PRJ_QUERY_FILE_NAME_CB QueryFileNameCallback;
        public PRJ_NOTIFICATION_CB NotificationCallback;
        public PRJ_CANCEL_COMMAND_CB CancelCommandCallback;
    }

    // ==========================================
    // P/Invoke Declarations
    // ==========================================

    public static class ProjFSNative
    {
        private const string ProjFSLib = "ProjectedFSLib.dll";

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjStartVirtualizing(
            string virtualizationRootPath,
            ref PRJ_CALLBACKS callbacks,
            IntPtr instanceContext,
            ref PRJ_STARTVIRTUALIZING_OPTIONS options,
            out IntPtr namespaceVirtualizationContext);

        [DllImport(ProjFSLib)]
        public static extern void PrjStopVirtualizing(IntPtr namespaceVirtualizationContext);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjClearNegativePathCache(
            IntPtr namespaceVirtualizationContext,
            out uint totalEntryNumber);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjWritePlaceholderInfo(
            IntPtr namespaceVirtualizationContext,
            string destinationFileName,
            ref PRJ_PLACEHOLDER_INFO placeholderInfo,
            uint placeholderInfoSize);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjWritePlaceholderInfo2(
            IntPtr namespaceVirtualizationContext,
            string destinationFileName,
            ref PRJ_PLACEHOLDER_INFO placeholderInfo,
            uint placeholderInfoSize,
            IntPtr extendedInfo);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjUpdateFileIfNeeded(
            IntPtr namespaceVirtualizationContext,
            string destinationFileName,
            ref PRJ_PLACEHOLDER_INFO placeholderInfo,
            uint placeholderInfoSize,
            UpdateType updateFlags,
            out UpdateFailureCause failureReason);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjDeleteFile(
            IntPtr namespaceVirtualizationContext,
            string destinationFileName,
            UpdateType updateFlags,
            out UpdateFailureCause failureReason);

        [DllImport(ProjFSLib)]
        public static extern IntPtr PrjAllocateAlignedBuffer(
            IntPtr namespaceVirtualizationContext,
            uint size);

        [DllImport(ProjFSLib)]
        public static extern void PrjFreeAlignedBuffer(IntPtr buffer);

        [DllImport(ProjFSLib)]
        public static extern HResult PrjWriteFileData(
            IntPtr namespaceVirtualizationContext,
            ref Guid dataStreamId,
            IntPtr buffer,
            ulong byteOffset,
            uint length);

        [DllImport(ProjFSLib)]
        public static extern HResult PrjCompleteCommand(
            IntPtr namespaceVirtualizationContext,
            int commandId,
            HResult completionResult,
            IntPtr extendedParameters);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjMarkDirectoryAsPlaceholder(
            string rootPathName,
            string targetPathName,
            ref PRJ_PLACEHOLDER_VERSION_INFO versionInfo,
            in Guid virtualizationInstanceID);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjFillDirEntryBuffer(
            string fileName,
            ref PRJ_FILE_BASIC_INFO fileBasicInfo,
            IntPtr dirEntryBufferHandle);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjFillDirEntryBuffer2(
            IntPtr dirEntryBufferHandle,
            string fileName,
            ref PRJ_FILE_BASIC_INFO fileBasicInfo,
            IntPtr extendedInfo);

        [DllImport(ProjFSLib, CharSet = CharSet.Unicode)]
        public static extern HResult PrjGetOnDiskFileState(
            string destinationFileName,
            out OnDiskFileState fileState);

        [DllImport(ProjFSLib)]
        public static extern bool PrjDoesNameContainWildCards(
            [MarshalAs(UnmanagedType.LPWStr)] string fileName);

        [DllImport(ProjFSLib)]
        public static extern int PrjFileNameCompare(
            [MarshalAs(UnmanagedType.LPWStr)] string fileName1,
            [MarshalAs(UnmanagedType.LPWStr)] string fileName2);

        [DllImport(ProjFSLib)]
        public static extern bool PrjFileNameMatch(
            [MarshalAs(UnmanagedType.LPWStr)] string fileNameToCheck,
            [MarshalAs(UnmanagedType.LPWStr)] string pattern);
    }
}
