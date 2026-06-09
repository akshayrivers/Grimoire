package EventLogger.network;

import EventLogger.model.Event;
import EventLogger.util.Config;

import java.io.PrintWriter;
import java.net.Socket;

public class NetworkClient {

    public void send(Event e) {

        try (
                Socket socket = new Socket(
                        Config.HOST,
                        Config.PORT);

                PrintWriter out = new PrintWriter(
                        socket.getOutputStream(),
                        true)) {

            out.println(e);

        } catch (Exception ex) {

            ex.printStackTrace();
        }
    }
}
