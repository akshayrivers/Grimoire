package EventLogger.consumer;

import EventLogger.model.Event;

import java.io.BufferedWriter;
import java.io.FileWriter;

public class FileConsumer implements LogListener {

    @Override
    public void onEvent(Event e) {
        try (
                BufferedWriter writer = new BufferedWriter(
                        new FileWriter(
                                "logs.txt",
                                true))) {

            writer.write(e.toString());
            writer.newLine();
            writer.flush();

            System.out.println("Written To File -> " + e);

        } catch (Exception ex) {
            ex.printStackTrace();
        }
    }
}
