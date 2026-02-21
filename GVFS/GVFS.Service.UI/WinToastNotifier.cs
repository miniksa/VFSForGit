using GVFS.Common;
using GVFS.Common.Tracing;
using System;
using System.IO;
using System.Xml;
using Windows.UI.Notifications;
using XmlDocument = Windows.Data.Xml.Dom.XmlDocument;

namespace GVFS.Service.UI
{
    public class WinToastNotifier : IToastNotifier
    {
        private const string ServiceAppId = "GVFS";
        private const string GVFSIconName = "GitVirtualFileSystem.ico";
        private ITracer tracer;

        public WinToastNotifier(ITracer tracer)
        {
            this.tracer = tracer;
        }

        public Action<string> UserResponseCallback { get; set; }

        public void Notify(string title, string message, string actionButtonTitle, string callbackArgs)
        {
            // Reference: https://docs.microsoft.com/en-us/windows/uwp/design/shell/tiles-and-notifications/adaptive-interactive-toasts
            // NativeAOT: Write toast XML directly instead of using XmlSerializer (which requires reflection).
            string logo = "file:///" + Path.Combine(ProcessHelper.GetCurrentProcessLocation(), GVFSIconName);

            XmlDocument toastXml = new XmlDocument();
            using (StringWriter stringWriter = new StringWriter())
            using (XmlWriter xmlWriter = XmlWriter.Create(stringWriter, new XmlWriterSettings { OmitXmlDeclaration = true }))
            {
                xmlWriter.WriteStartElement("toast");

                xmlWriter.WriteStartElement("visual");
                xmlWriter.WriteStartElement("binding");
                xmlWriter.WriteAttributeString("template", "ToastGeneric");

                xmlWriter.WriteElementString("text", title);
                xmlWriter.WriteElementString("text", message);

                xmlWriter.WriteStartElement("image");
                xmlWriter.WriteAttributeString("placement", "appLogoOverride");
                xmlWriter.WriteAttributeString("src", logo);
                xmlWriter.WriteAttributeString("hint-crop", "circle");
                xmlWriter.WriteEndElement(); // image

                xmlWriter.WriteEndElement(); // binding
                xmlWriter.WriteEndElement(); // visual

                if (!string.IsNullOrEmpty(actionButtonTitle))
                {
                    xmlWriter.WriteStartElement("actions");
                    xmlWriter.WriteStartElement("action");
                    xmlWriter.WriteAttributeString("content", actionButtonTitle);
                    xmlWriter.WriteAttributeString("arguments", string.IsNullOrEmpty(callbackArgs) ? string.Empty : callbackArgs);
                    xmlWriter.WriteAttributeString("activationtype", "background");
                    xmlWriter.WriteEndElement(); // action
                    xmlWriter.WriteEndElement(); // actions
                }

                xmlWriter.WriteEndElement(); // toast
                xmlWriter.Flush();

                toastXml.LoadXml(stringWriter.ToString());
            }

            ToastNotification toastNotification = new ToastNotification(toastXml);
            toastNotification.Activated += this.ToastActivated;
            toastNotification.Dismissed += this.ToastDismissed;
            toastNotification.Failed += this.ToastFailed;

            ToastNotifier toastNotifier = ToastNotificationManager.CreateToastNotifier(ServiceAppId);
            toastNotifier.Show(toastNotification);
        }

        private void ToastActivated(ToastNotification sender, object e)
        {
            ToastActivatedEventArgs args = (ToastActivatedEventArgs)e;

            this.UserResponseCallback?.Invoke(args.Arguments);
        }

        private void ToastDismissed(ToastNotification sender, ToastDismissedEventArgs e)
        {
            this.tracer.RelatedInfo($"{nameof(this.ToastDismissed)}: {e.Reason}");
        }

        private void ToastFailed(ToastNotification sender, ToastFailedEventArgs e)
        {
            this.tracer.RelatedInfo($"{nameof(this.ToastFailed)}: {e.ErrorCode.ToString()}");
        }
    }
}
