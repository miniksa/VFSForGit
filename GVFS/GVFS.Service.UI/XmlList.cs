using GVFS.Service.UI.Data;
using System;
using System.Collections.Generic;
using System.Xml;
using System.Xml.Schema;
using System.Xml.Serialization;

namespace GVFS.Service.UI
{
    public class XmlList<T> : List<T>, IXmlSerializable where T : class
    {
        public XmlSchema GetSchema()
        {
            throw new NotImplementedException();
        }

        public void ReadXml(XmlReader reader)
        {
            throw new NotImplementedException();
        }

        /// <summary>
        /// NativeAOT: Write XML elements directly instead of using XmlSerializer (which requires reflection).
        /// </summary>
        public void WriteXml(XmlWriter writer)
        {
            foreach (T item in this)
            {
                switch (item)
                {
                    case BindingItem.TextData text:
                        writer.WriteElementString("text", text.Value);
                        break;
                    case BindingItem.ImageData image:
                        writer.WriteStartElement("image");
                        if (image.Placement != null)
                        {
                            writer.WriteAttributeString("placement", image.Placement);
                        }

                        if (image.Source != null)
                        {
                            writer.WriteAttributeString("src", image.Source);
                        }

                        if (image.HintCrop != null)
                        {
                            writer.WriteAttributeString("hint-crop", image.HintCrop);
                        }

                        writer.WriteEndElement();
                        break;
                    case ActionItem action:
                        writer.WriteStartElement("action");
                        if (action.Content != null)
                        {
                            writer.WriteAttributeString("content", action.Content);
                        }

                        if (action.Arguments != null)
                        {
                            writer.WriteAttributeString("arguments", action.Arguments);
                        }

                        if (action.ActivationType != null)
                        {
                            writer.WriteAttributeString("activationtype", action.ActivationType);
                        }

                        writer.WriteEndElement();
                        break;
                    default:
                        throw new NotSupportedException($"XmlList does not support writing type {item.GetType().Name}");
                }
            }
        }
    }
}
