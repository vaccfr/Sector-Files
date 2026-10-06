// Compiled by Windows PowerShell's Add-Type; no third-party runtime is required.
// Preserve original JSON tokens (especially coordinate precision) when merging.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace Vsmr.Updates
{
    public sealed class MergeResult
    {
        public string Json;
        public string[] Conflicts;
    }

    public static class JsonThreeWayMerge
    {
        sealed class Node
        {
            public char Kind;
            public string Raw, Text;
            public Dictionary<string, Node> Properties;
            public List<string> Keys;
            public List<Node> Items;
        }

        sealed class Parser
        {
            readonly string source;
            int offset, nodes;
            public Parser(string text)
            {
                if (text == null || text.Length > 64 * 1024 * 1024)
                    throw new FormatException("JSON input exceeds the merge limit.");
                source = text;
            }
            void Space() { while (offset < source.Length && " \t\r\n".IndexOf(source[offset]) >= 0) ++offset; }
            char Take()
            {
                if (offset == source.Length) throw new FormatException("Truncated JSON.");
                return source[offset++];
            }
            void Expect(char value) { if (Take() != value) throw new FormatException("Invalid JSON delimiter."); }
            string ReadString()
            {
                Expect('"');
                var text = new StringBuilder();
                while (true)
                {
                    char c = Take();
                    if (c == '"')
                    {
                        string result = text.ToString();
                        for (int i = 0; i < result.Length; ++i)
                        {
                            if (char.IsHighSurrogate(result[i]))
                            {
                                if (++i == result.Length || !char.IsLowSurrogate(result[i]))
                                    throw new FormatException("Unpaired Unicode surrogate.");
                            }
                            else if (char.IsLowSurrogate(result[i])) throw new FormatException("Unpaired Unicode surrogate.");
                        }
                        return result;
                    }
                    if (c < 32) throw new FormatException("Control character in JSON string.");
                    if (c == '\\')
                    {
                        c = Take();
                        switch (c)
                        {
                            case '"': case '\\': case '/': break;
                            case 'b': c = '\b'; break;
                            case 'f': c = '\f'; break;
                            case 'n': c = '\n'; break;
                            case 'r': c = '\r'; break;
                            case 't': c = '\t'; break;
                            case 'u':
                                int code = 0;
                                for (int i = 0; i < 4; ++i)
                                {
                                    char h = Take();
                                    int digit = "0123456789abcdef".IndexOf(char.ToLowerInvariant(h));
                                    if (digit < 0) throw new FormatException("Invalid Unicode escape.");
                                    code = code * 16 + digit;
                                }
                                c = (char)code;
                                break;
                            default: throw new FormatException("Invalid JSON escape.");
                        }
                    }
                    text.Append(c);
                }
            }
            static bool Digit(char c) { return c >= '0' && c <= '9'; }
            void Digits()
            {
                int start = offset;
                while (offset < source.Length && Digit(source[offset])) ++offset;
                if (start == offset) throw new FormatException("Invalid JSON number.");
            }
            Node Value(int depth)
            {
                if (depth > 64 || ++nodes > 1000000) throw new FormatException("JSON merge complexity limit exceeded.");
                Space();
                int start = offset;
                if (offset == source.Length) throw new FormatException("Missing JSON value.");
                char c = source[offset];
                var node = new Node { Kind = c };
                if (c == '{')
                {
                    ++offset; Space();
                    node.Properties = new Dictionary<string, Node>(StringComparer.Ordinal);
                    node.Keys = new List<string>();
                    if (offset < source.Length && source[offset] != '}')
                    {
                        while (true)
                        {
                            Space(); string key = ReadString(); Space(); Expect(':');
                            if (node.Properties.ContainsKey(key)) throw new FormatException("Duplicate JSON object key.");
                            node.Properties.Add(key, Value(depth + 1)); node.Keys.Add(key); Space();
                            if (offset == source.Length || source[offset] != ',') break;
                            ++offset;
                        }
                    }
                    Expect('}');
                }
                else if (c == '[')
                {
                    ++offset; Space(); node.Items = new List<Node>();
                    if (offset < source.Length && source[offset] != ']')
                    {
                        while (true)
                        {
                            node.Items.Add(Value(depth + 1)); Space();
                            if (offset == source.Length || source[offset] != ',') break;
                            ++offset;
                        }
                    }
                    Expect(']');
                }
                else if (c == '"') node.Text = ReadString();
                else if (c == 't' || c == 'f' || c == 'n')
                {
                    string literal = c == 't' ? "true" : c == 'f' ? "false" : "null";
                    foreach (char ch in literal) Expect(ch);
                }
                else
                {
                    node.Kind = '0';
                    if (c == '-') ++offset;
                    if (offset < source.Length && source[offset] == '0') ++offset;
                    else Digits();
                    if (offset < source.Length && source[offset] == '.') { ++offset; Digits(); }
                    if (offset < source.Length && (source[offset] == 'e' || source[offset] == 'E'))
                    {
                        ++offset;
                        if (offset < source.Length && (source[offset] == '+' || source[offset] == '-')) ++offset;
                        Digits();
                    }
                }
                node.Raw = source.Substring(start, offset - start);
                return node;
            }
            public Node Parse()
            {
                Node result = Value(0); Space();
                if (offset != source.Length) throw new FormatException("Trailing JSON input.");
                return result;
            }
        }

        static Node Get(Node n, string key)
        {
            Node value;
            return n != null && n.Kind == '{' && n.Properties.TryGetValue(key, out value) ? value : null;
        }
        static bool Equal(Node a, Node b)
        {
            if (a == b) return true;
            if (a == null || b == null || a.Kind != b.Kind) return false;
            if (a.Raw != null && a.Raw == b.Raw) return true;
            if (a.Kind == '{')
            {
                if (a.Keys.Count != b.Keys.Count) return false;
                foreach (string key in a.Keys) if (!Equal(a.Properties[key], Get(b, key))) return false;
                return true;
            }
            if (a.Kind == '[')
            {
                if (a.Items.Count != b.Items.Count) return false;
                for (int i = 0; i < a.Items.Count; ++i) if (!Equal(a.Items[i], b.Items[i])) return false;
                return true;
            }
            return a.Kind == '"' ? a.Text == b.Text : a.Raw == b.Raw;
        }
        static string Pointer(string key) { return key.Replace("~", "~0").Replace("/", "~1"); }
        static List<string> Union(List<string> first, List<string> second)
        {
            var result = new List<string>(first);
            var seen = new HashSet<string>(first, StringComparer.Ordinal);
            foreach (string key in second) if (seen.Add(key)) result.Add(key);
            return result;
        }
        static string Identity(Node item, string key, bool profiles)
        {
            Node id = Get(item, key);
            if (id != null && (id.Kind == '"' || id.Kind == '0'))
                return key + ":" + (id.Kind == '"' ? id.Text : id.Raw);
            if (profiles && Get(item, "_vsmr") != null) return "metadata:_vsmr";
            return null;
        }
        static Node Index(Node array, string key, bool profiles)
        {
            var map = new Node { Kind = '{', Keys = new List<string>(), Properties = new Dictionary<string, Node>(StringComparer.Ordinal) };
            foreach (Node item in array.Items)
            {
                string id = Identity(item, key, profiles);
                if (id == null || map.Properties.ContainsKey(id)) return null;
                map.Keys.Add(id); map.Properties.Add(id, item);
            }
            return map;
        }
        static bool SameOrder(List<string> first, List<string> second, List<string> baseline)
        {
            var common = new HashSet<string>(baseline, StringComparer.Ordinal);
            common.IntersectWith(first); common.IntersectWith(second);
            var a = first.FindAll(common.Contains); var b = second.FindAll(common.Contains);
            if (a.Count != b.Count) return false;
            for (int i = 0; i < a.Count; ++i) if (a[i] != b[i]) return false;
            return true;
        }
        static Node Merge(Node baseline, Node local, Node incoming, string path, bool profiles, List<string> conflicts)
        {
            if (Equal(local, incoming) || Equal(incoming, baseline)) return local;
            if (Equal(local, baseline)) return incoming;
            if (!profiles && path.EndsWith("/geometry", StringComparison.Ordinal))
            { conflicts.Add(path + " (geometry conflict; user geometry retained)"); return local; }
            if (baseline != null && local != null && incoming != null &&
                baseline.Kind == '{' && local.Kind == '{' && incoming.Kind == '{')
            {
                // A schema change can redefine fields: do not mix edited old-schema objects with new ones.
                if (!Equal(Get(baseline, "schema_version"), Get(incoming, "schema_version")))
                { conflicts.Add(path + " (schema changed; user object retained)"); return local; }
                var merged = new Node { Kind = '{', Keys = new List<string>(), Properties = new Dictionary<string, Node>(StringComparer.Ordinal) };
                foreach (string key in Union(Union(local.Keys, incoming.Keys), baseline.Keys))
                {
                    Node value = Merge(Get(baseline, key), Get(local, key), Get(incoming, key), path + "/" + Pointer(key), profiles, conflicts);
                    if (value != null) { merged.Keys.Add(key); merged.Properties.Add(key, value); }
                }
                return merged;
            }
            if (baseline != null && local != null && incoming != null &&
                baseline.Kind == '[' && local.Kind == '[' && incoming.Kind == '[')
            {
                bool profileList = profiles && path == "";
                string key = profileList || (profiles && path.EndsWith("/display_modes/items", StringComparison.Ordinal)) ? "name" :
                    !profiles && (path == "/features" || path == "/vsmr_groups") ? "id" : null;
                if (key != null)
                {
                    Node b = Index(baseline, key, profileList), l = Index(local, key, profileList), r = Index(incoming, key, profileList);
                    if (b != null && l != null && r != null)
                    {
                        bool localOrder = !SameOrder(l.Keys, b.Keys, b.Keys);
                        if (localOrder && !SameOrder(r.Keys, b.Keys, b.Keys) && !SameOrder(l.Keys, r.Keys, b.Keys))
                            conflicts.Add(path + " (order conflict; user order retained)");
                        var merged = new Node { Kind = '[', Items = new List<Node>() };
                        var order = localOrder ? Union(l.Keys, r.Keys) : Union(r.Keys, l.Keys);
                        foreach (string id in order)
                        {
                            Node value = Merge(Get(b, id), Get(l, id), Get(r, id), path + "/" + Pointer(id), profiles, conflicts);
                            if (value != null) merged.Items.Add(value);
                        }
                        return merged;
                    }
                }
            }
            // Missing nodes mean deletion, distinct from an explicit JSON null.
            // Geometry, ordered rules and ambiguous arrays are atomic: never merge by array position.
            conflicts.Add(path + " (both changed; user value retained)");
            return local;
        }
        static void Quote(StringBuilder output, string text)
        {
            output.Append('"');
            foreach (char c in text)
            {
                if (c == '"' || c == '\\') output.Append('\\').Append(c);
                else if (c < 32) output.Append("\\u").Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
                else output.Append(c);
            }
            output.Append('"');
        }
        static void Write(Node node, StringBuilder output)
        {
            if (node.Raw != null) { output.Append(node.Raw); return; }
            bool first = true;
            output.Append(node.Kind);
            if (node.Kind == '{')
            {
                foreach (string key in node.Keys)
                {
                    if (!first) output.Append(','); first = false;
                    Quote(output, key); output.Append(':'); Write(node.Properties[key], output);
                }
                output.Append('}');
            }
            else
            {
                foreach (Node value in node.Items) { if (!first) output.Append(','); first = false; Write(value, output); }
                output.Append(']');
            }
        }
        public static MergeResult MergeJson(string baseline, string local, string incoming, bool profiles)
        {
            Node b = new Parser(baseline).Parse(), l = new Parser(local).Parse(), r = new Parser(incoming).Parse();
            char expected = profiles ? '[' : '{';
            if (b.Kind != expected || l.Kind != expected || r.Kind != expected)
                throw new FormatException("Unexpected map/profile document type.");
            var conflicts = new List<string>();
            Node merged = Merge(b, l, r, "", profiles, conflicts);
            if (!profiles && !ValidMapReferences(merged))
            {
                conflicts.Add("Map references became ambiguous; complete user map retained.");
                merged = l;
            }
            var json = new StringBuilder(); Write(merged, json);
            return new MergeResult { Json = json.ToString(), Conflicts = conflicts.ToArray() };
        }

        static bool ValidMapReferences(Node map)
        {
            Node features = Get(map, "features"), styles = Get(map, "styles"), groups = Get(map, "vsmr_groups");
            if (features == null || features.Kind != '[') return true;
            var groupIds = new HashSet<string>(StringComparer.Ordinal);
            if (groups != null && groups.Kind == '[')
                foreach (Node group in groups.Items)
                {
                    Node id = Get(group, "id");
                    if (id != null && id.Kind == '"') groupIds.Add(id.Text);
                }
            foreach (Node feature in features.Items)
            {
                Node properties = Get(feature, "properties"), style = Get(properties, "style_id");
                if (style != null && style.Kind == '"' && Get(styles, style.Text) == null) return false;
                Node ids = Get(properties, "vsmr_group_ids");
                if (ids != null && ids.Kind == '[')
                    foreach (Node id in ids.Items)
                        if (id.Kind != '"' || !groupIds.Contains(id.Text)) return false;
            }
            return true;
        }
    }
}
