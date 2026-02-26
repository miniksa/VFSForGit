using GVFS.Common;
using GVFS.Common.Database;
using GVFS.Common.FileSystem;
using GVFS.Common.Git;
using GVFS.Common.Tracing;
using GVFS.Tests.Should;
using GVFS.UnitTests.Mock.Common;
using GVFS.UnitTests.Mock.Virtualization.Background;
using GVFS.UnitTests.Mock.Git;
using GVFS.UnitTests.Mock.Virtualization.Background;
using GVFS.UnitTests.Mock.Virtualization.BlobSize;
using GVFS.UnitTests.Mock.Virtualization.FileSystem;
using GVFS.UnitTests.Mock.Virtualization.Projection;
using GVFS.Virtualization;
using GVFS.Virtualization.Background;
using Moq;
using NUnit.Framework;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace GVFS.UnitTests.Virtualization
{
    /// <summary>
    /// Tests for TryPreCheckoutDehydrate.
    ///
    /// These tests use a real temp directory on disk because the dehydrate
    /// code uses File.Delete directly (not the virtualizer API), so we need
    /// actual files for the deletion to work.
    /// </summary>
    [TestFixture]
    public class PreCheckoutDehydrateTests : IDisposable
    {
        private string tempRoot;
        private string workingDir;
        private string dotGvfsDir;
        private MockTracer tracer;

        [SetUp]
        public void SetUp()
        {
            this.tempRoot = Path.Combine(Path.GetTempPath(), "GVFS_PreCheckoutDehydrate_" + Guid.NewGuid().ToString("N"));
            this.workingDir = Path.Combine(this.tempRoot, "src");
            this.dotGvfsDir = Path.Combine(this.tempRoot, GVFSPlatform.Instance.Constants.DotGVFSRoot);
            Directory.CreateDirectory(this.workingDir);
            Directory.CreateDirectory(this.dotGvfsDir);
            Directory.CreateDirectory(Path.Combine(this.dotGvfsDir, "databases"));

            // Create minimal git structure so GVFSEnlistment doesn't complain
            string gitDir = Path.Combine(this.workingDir, ".git");
            Directory.CreateDirectory(Path.Combine(gitDir, "logs"));
            Directory.CreateDirectory(Path.Combine(gitDir, "info"));
            Directory.CreateDirectory(Path.Combine(gitDir, "objects", "pack"));
            File.WriteAllText(Path.Combine(gitDir, "config"), "[core]\n\trepositoryformatversion = 0\n");
            File.WriteAllText(Path.Combine(gitDir, "HEAD"), "ref: refs/heads/main\n");
            File.WriteAllText(Path.Combine(gitDir, "logs", "HEAD"), "log\n");
            File.WriteAllText(Path.Combine(gitDir, "info", "always_exclude"), "\n");

            this.tracer = new MockTracer();

            string error;
            RepoMetadata.TryInitialize(this.tracer, new PhysicalFileSystem(), this.dotGvfsDir, out error);
        }

        [TearDown]
        public void TearDown()
        {
            RepoMetadata.Shutdown();
            this.Dispose();
        }

        public void Dispose()
        {
            if (this.tempRoot != null && Directory.Exists(this.tempRoot))
            {
                try
                {
                    Directory.Delete(this.tempRoot, recursive: true);
                }
                catch
                {
                    // Best effort cleanup
                }
            }
        }

        [Test]
        public void DehydrateDeletesFilesAndShrinkModifiedPaths()
        {
            // Arrange: create files on disk and add them to ModifiedPaths
            string[] filePaths = new[]
            {
                "src/file1.c",
                "src/file2.c",
                "tools/helper.py",
            };

            foreach (string path in filePaths)
            {
                string fullPath = Path.Combine(this.workingDir, path.Replace('/', Path.DirectorySeparatorChar));
                Directory.CreateDirectory(Path.GetDirectoryName(fullPath));
                File.WriteAllText(fullPath, "test content");
            }

            using (FileSystemCallbacks fsc = this.CreateFileSystemCallbacks(filePaths))
            {
                // Verify initial state
                fsc.GetAllModifiedPaths().Count().ShouldBeAtLeast(filePaths.Length);

                // Act
                FileSystemCallbacks.PreCheckoutDehydrateResult result = fsc.TryPreCheckoutDehydrate();

                // Assert
                result.FilesDehydrated.ShouldEqual(filePaths.Length);
                result.FilesFailed.ShouldEqual(0);
                result.ElapsedMs.ShouldBeAtLeast(0);

                // Files should be deleted from disk
                foreach (string path in filePaths)
                {
                    string fullPath = Path.Combine(this.workingDir, path.Replace('/', Path.DirectorySeparatorChar));
                    File.Exists(fullPath).ShouldEqual(false);
                }

                // ModifiedPaths should be reduced to just .gitattributes
                List<string> remaining = fsc.GetAllModifiedPaths().ToList();
                remaining.Count.ShouldEqual(1);
                remaining[0].ShouldEqual(GVFSConstants.SpecialGitFiles.GitAttributes);
            }
        }

        [Test]
        public void DehydratePreservesGitAttributes()
        {
            // Arrange: only .gitattributes in ModifiedPaths (the seed entry)
            using (FileSystemCallbacks fsc = this.CreateFileSystemCallbacks(Array.Empty<string>()))
            {
                // Act
                FileSystemCallbacks.PreCheckoutDehydrateResult result = fsc.TryPreCheckoutDehydrate();

                // Assert
                result.FilesDehydrated.ShouldEqual(0);
                result.FilesFailed.ShouldEqual(0);

                // .gitattributes should still be present
                fsc.GetAllModifiedPaths().ShouldContain(p => p == GVFSConstants.SpecialGitFiles.GitAttributes);
            }
        }

        [Test]
        public void DehydrateHandlesMissingFiles()
        {
            // Arrange: entries in ModifiedPaths but files DON'T exist on disk
            string[] filePaths = new[] { "src/deleted.c", "tools/removed.py" };

            // Don't create the files — they're "already gone"
            using (FileSystemCallbacks fsc = this.CreateFileSystemCallbacks(filePaths))
            {
                // Act
                FileSystemCallbacks.PreCheckoutDehydrateResult result = fsc.TryPreCheckoutDehydrate();

                // Assert: should succeed — missing files are counted as dehydrated
                result.FilesDehydrated.ShouldEqual(filePaths.Length);
                result.FilesFailed.ShouldEqual(0);

                // ModifiedPaths should be down to just .gitattributes
                fsc.GetAllModifiedPaths().Count().ShouldEqual(1);
            }
        }

        [Test]
        public void DehydrateHandlesReadOnlyFiles()
        {
            // Arrange: create a read-only file
            string path = "src/readonly.h";
            string fullPath = Path.Combine(this.workingDir, path.Replace('/', Path.DirectorySeparatorChar));
            Directory.CreateDirectory(Path.GetDirectoryName(fullPath));
            File.WriteAllText(fullPath, "readonly content");
            File.SetAttributes(fullPath, FileAttributes.ReadOnly);

            using (FileSystemCallbacks fsc = this.CreateFileSystemCallbacks(new[] { path }))
            {
                // Act
                FileSystemCallbacks.PreCheckoutDehydrateResult result = fsc.TryPreCheckoutDehydrate();

                // Assert: read-only files should still be deleted (we clear the attribute)
                result.FilesDehydrated.ShouldEqual(1);
                result.FilesFailed.ShouldEqual(0);
                File.Exists(fullPath).ShouldEqual(false);
            }
        }

        [Test]
        public void DehydrateSkipsFolderEntries()
        {
            // Arrange: add a folder entry to ModifiedPaths
            string[] filePaths = new[] { "src/file.c" };
            string fullPath = Path.Combine(this.workingDir, "src", "file.c");
            Directory.CreateDirectory(Path.GetDirectoryName(fullPath));
            File.WriteAllText(fullPath, "content");

            using (FileSystemCallbacks fsc = this.CreateFileSystemCallbacks(filePaths, folderEntries: new[] { "src/" }))
            {
                int initialCount = fsc.GetAllModifiedPaths().Count();

                // Act
                FileSystemCallbacks.PreCheckoutDehydrateResult result = fsc.TryPreCheckoutDehydrate();

                // Assert: file dehydrated, folder entry removed as orphan
                result.FilesDehydrated.ShouldEqual(1);
                File.Exists(fullPath).ShouldEqual(false);
            }
        }

        [Test]
        public void DehydrateReportsElapsedTime()
        {
            string[] filePaths = new[] { "src/a.c", "src/b.c" };
            foreach (string path in filePaths)
            {
                string fullPath = Path.Combine(this.workingDir, path.Replace('/', Path.DirectorySeparatorChar));
                Directory.CreateDirectory(Path.GetDirectoryName(fullPath));
                File.WriteAllText(fullPath, "content");
            }

            using (FileSystemCallbacks fsc = this.CreateFileSystemCallbacks(filePaths))
            {
                FileSystemCallbacks.PreCheckoutDehydrateResult result = fsc.TryPreCheckoutDehydrate();
                result.ElapsedMs.ShouldBeAtLeast(0);
                result.Success.ShouldEqual(true);
            }
        }

        private FileSystemCallbacks CreateFileSystemCallbacks(string[] filePaths, string[] folderEntries = null)
        {
            GVFSEnlistment enlistment = new GVFSEnlistment(this.tempRoot, "fake://repoUrl", "fake://gitBinPath", authentication: null);
            enlistment.InitializeCachePathsFromKey(Path.Combine(this.tempRoot, "cache"), "fakeCacheKey");

            // Ensure BlobSizes directory exists
            Directory.CreateDirectory(enlistment.BlobSizesRoot);

            PhysicalFileSystem fileSystem = new PhysicalFileSystem();

            // Pre-populate the ModifiedPaths.dat file so FileSystemCallbacks loads it
            string modifiedPathsFile = Path.Combine(this.dotGvfsDir, GVFSConstants.DotGVFS.Databases.ModifiedPaths);
            Directory.CreateDirectory(Path.GetDirectoryName(modifiedPathsFile));
            using (StreamWriter writer = new StreamWriter(modifiedPathsFile))
            {
                // .gitattributes is always present as the seed entry
                writer.WriteLine("A " + GVFSConstants.SpecialGitFiles.GitAttributes);

                foreach (string path in filePaths)
                {
                    writer.WriteLine("A " + path);
                }

                if (folderEntries != null)
                {
                    foreach (string folder in folderEntries)
                    {
                        writer.WriteLine("A " + folder);
                    }
                }
            }

            GVFSContext context = new GVFSContext(this.tracer, fileSystem, null, enlistment);

            Mock<IPlaceholderCollection> mockPlaceholderDb = new Mock<IPlaceholderCollection>(MockBehavior.Loose);
            mockPlaceholderDb.Setup(x => x.GetCount()).Returns(0);
            mockPlaceholderDb.Setup(x => x.GetAllFilePaths()).Returns(new HashSet<string>());

            Mock<ISparseCollection> mockSparseDb = new Mock<ISparseCollection>(MockBehavior.Strict);
            mockSparseDb.Setup(x => x.GetAll()).Returns(new HashSet<string>());

            MockBackgroundFileSystemTaskRunner backgroundTaskRunner = new MockBackgroundFileSystemTaskRunner();

            return new FileSystemCallbacks(
                context,
                new MockGVFSGitObjects(context, new MockHttpGitObjects(this.tracer, enlistment)),
                RepoMetadata.Instance,
                new MockBlobSizes(),
                gitIndexProjection: null,
                backgroundFileSystemTaskRunner: backgroundTaskRunner,
                fileSystemVirtualizer: null,
                placeholderDatabase: mockPlaceholderDb.Object,
                sparseCollection: mockSparseDb.Object);
        }
    }
}
